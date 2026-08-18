//! Ingestor.
//!
//! Polls each source on its venue's own schedule, publishes normalised events
//! to the bus, and batches them into the archive.
//!
//! Two decisions worth stating, because both look like premature optimisation
//! and are not:
//!
//! **Polling follows the venue calendar, not a fixed interval.** A source
//! polled at 3am WAT returns yesterday's close and costs a request against a
//! free service's rate limit. Outside a session the loop idles.
//!
//! **The archive write is a diff, not a snapshot.** These venues are thin
//! enough that most poll cycles return an unchanged board — NGX clears ~42,000
//! deals across a seven-hour session spread over ~150 tickers, so the long tail
//! prints a handful of times a day. Archiving every poll would fill the tick
//! store with duplicates and make "when did this actually change" unanswerable.

use std::collections::HashMap;
use std::time::Duration;

use amd_adapters::http::HttpClient;
use amd_adapters::{Adapter, AdapterRegistry};
use amd_bus::{Bus, BusConfig, Event};
use amd_calendar::{SessionState, session_state, venue};
use amd_core::{ExchangeCode, InstrumentId, Price, Quote};
use amd_store::{ClickhouseStore, QuoteRow};
use time::OffsetDateTime;
use tracing::{debug, error, info, warn};
use tracing_subscriber::EnvFilter;

/// How often to poll while a venue is open.
const OPEN_INTERVAL: Duration = Duration::from_secs(30);
/// How often to re-check the clock while a venue is closed.
const CLOSED_INTERVAL: Duration = Duration::from_secs(300);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let registry = AdapterRegistry::with_defaults(HttpClient::new()?);

    let bus = match std::env::var("NATS_URL") {
        Ok(url) => match Bus::connect(&BusConfig {
            url: url.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(b) => Some(b),
            Err(e) => {
                warn!(%url, error = %e, "bus unavailable; running without fan-out");
                None
            }
        },
        Err(_) => None,
    };

    let archive = match std::env::var("CLICKHOUSE_URL") {
        Ok(url) => {
            let s = ClickhouseStore::new(
                &url,
                &std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "amd".into()),
                &std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
                &std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "amd".into()),
            );
            match s.ping().await {
                Ok(()) => Some(s),
                Err(e) => {
                    warn!(%url, error = %e, "archive unavailable; running without persistence");
                    None
                }
            }
        }
        Err(_) => None,
    };

    // One task per venue. Venues fail independently: a Ghanaian outage must not
    // stall Nigerian ingestion.
    let mut tasks = Vec::new();
    for exchange in registry.supported() {
        let Some(adapter) = registry.preferred(exchange).cloned() else {
            continue;
        };
        let bus = bus.clone();
        let archive = archive.clone();
        tasks.push(tokio::spawn(async move {
            run_venue(exchange, adapter, bus, archive).await;
        }));
    }

    if tasks.is_empty() {
        error!("no adapters registered; nothing to ingest");
        return Ok(());
    }
    info!(venues = tasks.len(), "ingestor started");

    tokio::signal::ctrl_c().await?;
    info!("shutting down");
    Ok(())
}

fn force_poll() -> bool {
    matches!(std::env::var("AMD_FORCE_POLL").as_deref(), Ok("1" | "true"))
}

/// Last published price per instrument, used to suppress unchanged rows.
type LastSeen = HashMap<InstrumentId, Price>;

async fn run_venue(
    exchange: ExchangeCode,
    adapter: std::sync::Arc<dyn Adapter>,
    bus: Option<Bus>,
    archive: Option<ClickhouseStore>,
) {
    let v = venue(exchange);
    let mut last_seen: LastSeen = HashMap::new();
    let mut last_state: Option<SessionState> = None;

    loop {
        let now = OffsetDateTime::now_utc();
        let state = session_state(now, v).unwrap_or(SessionState::Closed);

        if last_state != Some(state) {
            info!(%exchange, ?state, "session state changed");
            if let Some(b) = &bus {
                let ev = Event::Status {
                    exchange,
                    state: format!("{state:?}").to_lowercase(),
                };
                if let Err(e) = b.publish(&ev).await {
                    warn!(%exchange, error = %e, "status publish failed");
                }
            }
            // A new session starts from a clean slate: yesterday's closes are
            // not a baseline for today's first prints.
            if state == SessionState::PreOpen {
                last_seen.clear();
            }
            last_state = Some(state);
        }

        // AMD_FORCE_POLL exists so the pipeline can be exercised outside market
        // hours — every African venue is closed for most of a working day in
        // Europe or the Americas, and a data path that can only be tested at
        // 10am WAT does not get tested. Never set this in production: it polls
        // free sources around the clock for data that cannot have changed.
        if state != SessionState::Open && !force_poll() {
            tokio::time::sleep(CLOSED_INTERVAL).await;
            continue;
        }

        match adapter.quotes(exchange, &[]).await {
            Ok(quotes) => {
                let changed = poll_once(&quotes, &mut last_seen, &bus, &archive).await;
                debug!(%exchange, total = quotes.len(), changed, "polled");
            }
            Err(e) => warn!(%exchange, source = adapter.id(), error = %e, "poll failed"),
        }

        tokio::time::sleep(OPEN_INTERVAL).await;
    }
}

/// Publish and archive only what actually moved. Returns the changed count.
async fn poll_once(
    quotes: &[Quote],
    last_seen: &mut LastSeen,
    bus: &Option<Bus>,
    archive: &Option<ClickhouseStore>,
) -> usize {
    let mut rows = Vec::new();
    let mut changed = 0;

    for q in quotes {
        let Some(last) = q.last else { continue };
        // Compare on the fixed-point value, not a formatted string: a price
        // that rescales identically has not moved.
        let moved = match last_seen.get(&q.instrument) {
            Some(prev) => prev.cmp_value(last).map(|o| o.is_ne()).unwrap_or(true),
            None => true,
        };
        if !moved {
            continue;
        }
        last_seen.insert(q.instrument.clone(), last);
        changed += 1;

        if let Some(b) = bus
            && let Err(e) = b.publish(&Event::Quote(q.clone())).await
        {
            warn!(instrument = %q.instrument, error = %e, "publish failed");
        }

        if archive.is_some() {
            match QuoteRow::from_quote(q) {
                Ok(row) => rows.push(row),
                Err(e) => warn!(instrument = %q.instrument, error = %e, "not archivable"),
            }
        }
    }

    if let Some(a) = archive
        && !rows.is_empty()
    {
        match a.insert_quotes(&rows).await {
            Ok(n) => debug!(rows = n, "archived"),
            Err(e) => error!(error = %e, rows = rows.len(), "archive write failed"),
        }
    }
    changed
}
