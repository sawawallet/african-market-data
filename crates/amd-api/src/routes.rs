//! HTTP surface.
//!
//! Every quote response carries its provenance. That is not a debugging
//! convenience: a consumer who cannot see `as_of` and `delay` will read a
//! two-hour-old price on a thin venue as "the market is quiet today", and the
//! API is the last place able to prevent that.

use std::convert::Infallible;
use std::time::Duration;

use amd_calendar::{SessionState, VENUES, session_state, staleness_seconds, unverified, venue};
use amd_core::{DelayClass, ExchangeCode, Quote};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::entitlement::{self, Withheld};
use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/exchanges", get(list_exchanges))
        .route("/v1/sources", get(list_sources))
        .route("/v1/exchanges/{code}/instruments", get(list_instruments))
        .route("/v1/exchanges/{code}/quotes", get(list_quotes))
        .route("/v1/exchanges/{code}/stream", get(stream_quotes))
        .with_state(state)
}

// ---------------------------------------------------------------- errors

pub struct ApiError {
    status: StatusCode,
    message: String,
    /// What the caller can do about it. Absent when there is nothing useful.
    hint: Option<String>,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        ApiError {
            status,
            message: message.into(),
            hint: None,
        }
    }

    fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    fn bad_request(msg: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, msg)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({
            "error": self.message,
            "hint": self.hint,
        });
        (self.status, Json(body)).into_response()
    }
}

fn parse_exchange(code: &str) -> Result<ExchangeCode, ApiError> {
    code.parse().map_err(|_| {
        ApiError::bad_request(format!("unknown exchange: {code}")).with_hint(format!(
            "supported: {}",
            ExchangeCode::ALL.map(|c| c.as_str()).join(", ")
        ))
    })
}

// ---------------------------------------------------------------- health

#[derive(Serialize)]
struct Health {
    status: &'static str,
    version: &'static str,
    adapters: usize,
    bus: bool,
    archive: bool,
}

async fn health(State(s): State<AppState>) -> Json<Health> {
    Json(Health {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        adapters: s.adapters.all().len(),
        bus: s.bus.is_some(),
        archive: s.archive.is_some(),
    })
}

// ---------------------------------------------------------------- exchanges

#[derive(Serialize)]
struct ExchangeView {
    code: &'static str,
    name: &'static str,
    countries: &'static [&'static str],
    currency: String,
    timezone: &'static str,
    sessions: Vec<SessionView>,
    session_state: String,
    /// Whether the published session times have been checked against the venue
    /// itself. Surfaced rather than hidden: an unverified schedule is a caveat
    /// the consumer is entitled to know about.
    sessions_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    sessions_source: Option<&'static str>,
    /// True when this deployment has an adapter that can serve the venue.
    served: bool,
}

#[derive(Serialize)]
struct SessionView {
    open: String,
    close: String,
}

fn hhmm(minutes: u16) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

async fn list_exchanges(State(s): State<AppState>) -> Json<serde_json::Value> {
    let now = OffsetDateTime::now_utc();
    let served = s.adapters.supported();
    let items: Vec<ExchangeView> = VENUES
        .iter()
        .map(|v| ExchangeView {
            code: v.code.as_str(),
            name: v.name,
            countries: v.countries,
            currency: v.currency.to_string(),
            timezone: v.timezone,
            sessions: v
                .sessions
                .iter()
                .map(|w| SessionView {
                    open: hhmm(w.open_min),
                    close: hhmm(w.close_min),
                })
                .collect(),
            session_state: match session_state(now, v) {
                Ok(SessionState::Open) => "open",
                Ok(SessionState::PreOpen) => "pre-open",
                Ok(SessionState::Closed) => "closed",
                Err(_) => "unknown",
            }
            .to_owned(),
            sessions_verified: v.sessions_verified,
            sessions_source: v.sessions_source,
            served: served.contains(&v.code),
        })
        .collect();

    Json(serde_json::json!({
        "exchanges": items,
        "note": format!(
            "{} of {} venues have session times verified against the exchange's own \
             schedule; the rest are best-effort. Public holidays are not modelled.",
            VENUES.len() - unverified().count(),
            VENUES.len()
        ),
    }))
}

// ---------------------------------------------------------------- sources

#[derive(Serialize)]
struct SourceView {
    id: &'static str,
    exchanges: Vec<&'static str>,
    terms: &'static str,
    provides_timestamps: bool,
    provides_history: bool,
}

async fn list_sources(State(s): State<AppState>) -> Json<serde_json::Value> {
    let sources: Vec<SourceView> = s
        .adapters
        .all()
        .iter()
        .map(|a| {
            let caps = a.capabilities();
            SourceView {
                id: a.id(),
                exchanges: a.exchanges().iter().map(|e| e.as_str()).collect(),
                terms: a.terms(),
                provides_timestamps: caps.timestamps,
                provides_history: caps.history,
            }
        })
        .collect();
    Json(serde_json::json!({ "sources": sources }))
}

// ---------------------------------------------------------------- instruments

async fn list_instruments(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let exchange = parse_exchange(&code)?;
    let adapter = s.adapters.preferred(exchange).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            format!("no source registered for {exchange}"),
        )
        .with_hint(format!("served: {:?}", s.adapters.supported()))
    })?;

    let instruments = adapter
        .list_instruments(exchange)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, e.to_string()))?;

    Ok(Json(serde_json::json!({
        "exchange": exchange.as_str(),
        "source": adapter.id(),
        "count": instruments.len(),
        "instruments": instruments,
    })))
}

// ---------------------------------------------------------------- quotes

#[derive(Debug, Deserialize)]
pub struct QuoteQuery {
    /// Comma-separated tickers. Omit for the whole board.
    symbols: Option<String>,
    /// Seconds after which an open-session quote is flagged stale.
    #[serde(default = "default_stale_after")]
    stale_after: i64,
}

fn default_stale_after() -> i64 {
    // Roughly where a quiet NGX or GSE ticker stops being distinguishable from
    // a broken feed.
    900
}

#[derive(Serialize)]
struct AnnotatedQuote {
    #[serde(flatten)]
    quote: Quote,
    staleness_seconds: i64,
    /// True when the venue is open but this datum has aged past `stale_after`.
    /// The signal separating "nobody is trading it" from "the feed is broken".
    stale: bool,
}

async fn list_quotes(
    State(s): State<AppState>,
    Path(code): Path<String>,
    Query(q): Query<QuoteQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let exchange = parse_exchange(&code)?;
    let adapter = s.adapters.preferred(exchange).ok_or_else(|| {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            format!("no source registered for {exchange}"),
        )
    })?;

    let symbols: Vec<String> = q
        .symbols
        .as_deref()
        .map(|s| {
            s.split(',')
                .map(|t| t.trim().to_ascii_uppercase())
                .filter(|t| !t.is_empty())
                .collect()
        })
        .unwrap_or_default();

    let quotes = adapter
        .quotes(exchange, &symbols)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, e.to_string()))?;

    // Entitlements are unwired until a key store exists; anonymous rules apply.
    let (quotes, withheld): (Vec<Quote>, Withheld) = entitlement::filter(quotes, None);

    let now = OffsetDateTime::now_utc();
    let v = venue(exchange);
    let state = session_state(now, v).unwrap_or(SessionState::Closed);

    let annotated: Vec<AnnotatedQuote> = quotes
        .into_iter()
        .map(|quote| {
            let age = staleness_seconds(quote.provenance.as_of, now);
            AnnotatedQuote {
                stale: state == SessionState::Open && age > q.stale_after,
                staleness_seconds: age,
                quote,
            }
        })
        .collect();

    Ok(Json(serde_json::json!({
        "exchange": exchange.as_str(),
        "source": adapter.id(),
        "session_state": match state {
            SessionState::Open => "open",
            SessionState::PreOpen => "pre-open",
            SessionState::Closed => "closed",
        },
        "count": annotated.len(),
        "withheld": withheld,
        "quotes": annotated,
    })))
}

// ---------------------------------------------------------------- stream

/// Server-Sent Events rather than WebSocket.
///
/// Price updates are one-way. SSE auto-reconnects, survives corporate proxies
/// and needs no heartbeat protocol of its own. WebSocket only earns its
/// complexity once there is order entry to carry back.
async fn stream_quotes(
    State(s): State<AppState>,
    Path(code): Path<String>,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    let exchange = parse_exchange(&code)?;
    let bus = s.bus.clone().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "streaming requires the event bus",
        )
        .with_hint("start NATS and set NATS_URL, or poll /quotes instead")
    })?;

    let subject = amd_bus::subject::kind_wildcard(exchange, amd_bus::Kind::Quote);
    let events = bus
        .subscribe(&subject)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, e.to_string()))?;

    let stream = events.filter_map(|ev| async move {
        let amd_bus::Event::Quote(q) = ev else {
            return None;
        };
        // Entitlement filtering applies to the stream exactly as it does to
        // the snapshot endpoint; a subscription is not a way around it.
        if q.provenance.delay != DelayClass::Unknown
            && !entitlement::ANONYMOUS_MAX.permits(q.provenance.delay)
        {
            return None;
        }
        serde_json::to_string(&q)
            .ok()
            .map(|json| Ok(SseEvent::default().event("quote").data(json)))
    });

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}
