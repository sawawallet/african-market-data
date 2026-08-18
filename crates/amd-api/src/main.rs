//! African market data API.
//!
//! Boots with whatever infrastructure is actually present. Adapters alone are
//! enough to serve quotes, so a fresh clone runs with no dependencies at all;
//! NATS, ClickHouse and Postgres each enable more of the surface as they
//! appear. Degrading loudly in the logs but continuing to serve is deliberate —
//! a market data API that refuses to start because its archive is down is
//! worse than one that keeps quoting.

use std::net::SocketAddr;

use amd_adapters::AdapterRegistry;
use amd_adapters::http::HttpClient;
use amd_api::routes;
use amd_api::state::AppState;
use amd_bus::{Bus, BusConfig};
use amd_store::ClickhouseStore;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let http = HttpClient::new()?;
    let mut state = AppState::adapters_only(AdapterRegistry::with_defaults(http));

    if let Ok(url) = std::env::var("NATS_URL") {
        match Bus::connect(&BusConfig {
            url: url.clone(),
            ..Default::default()
        })
        .await
        {
            Ok(bus) => {
                info!(%url, "bus connected; streaming enabled");
                state.bus = Some(bus);
            }
            // Not fatal: /quotes still works, /stream returns 503 with a hint.
            Err(e) => warn!(%url, error = %e, "bus unavailable; streaming disabled"),
        }
    }

    if let Ok(url) = std::env::var("CLICKHOUSE_URL") {
        let store = ClickhouseStore::new(
            &url,
            &std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "amd".into()),
            &std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
            &std::env::var("CLICKHOUSE_DATABASE").unwrap_or_else(|_| "amd".into()),
        );
        match store.ping().await {
            Ok(()) => {
                info!(%url, "archive connected");
                state.archive = Some(store);
            }
            Err(e) => warn!(%url, error = %e, "archive unavailable; history disabled"),
        }
    }

    let app = routes::router(state)
        .layer(TraceLayer::new_for_http())
        .layer(CompressionLayer::new())
        // Wide-open CORS: this API serves public market data to browsers, and
        // entitlement enforcement happens per key, not per origin.
        .layer(CorsLayer::permissive());

    let bind: SocketAddr = std::env::var("AMD_API_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()?;
    let listener = tokio::net::TcpListener::bind(bind).await?;
    info!(%bind, "listening");
    axum::serve(listener, app).await?;
    Ok(())
}
