//! Source adapters.
//!
//! Adding a venue means implementing [`Adapter`] and registering it. Adapters
//! do not decide policy: they report what their source actually provides,
//! including an honest [`DelayClass`](amd_core::DelayClass) and an honest
//! `as_of_imputed`. An adapter that claims `Realtime` for a source that does
//! not guarantee it is a bug, not optimism — the entitlement checks in
//! `amd-api` trust that field.
//!
//! **Only sources that are unambiguously free to use belong in this crate.**
//! This is a public repository and it must not ship code that scrapes exchange
//! websites, nor imply an entitlement nobody holds. Licensed feeds — NGX direct,
//! JSE MITCH, commercial aggregators — are implemented in `amd-feed` and
//! registered at runtime by the operator who holds the licence.

pub mod http;
pub mod kwayisi;
pub mod registry;

use amd_core::{Bar, ExchangeCode, Instrument, Quote};
use async_trait::async_trait;

pub use registry::AdapterRegistry;

#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("{source_id} does not serve {exchange}")]
    UnsupportedExchange {
        source_id: &'static str,
        exchange: ExchangeCode,
    },
    #[error("{source_id} request failed: {0}", .message)]
    Request {
        source_id: &'static str,
        message: String,
    },
    #[error("{source_id} returned data this adapter could not interpret: {message}")]
    Malformed {
        source_id: &'static str,
        message: String,
    },
    #[error("{source_id} does not provide history")]
    NoHistory { source_id: &'static str },
}

/// What an adapter can do beyond serving quotes, so callers can degrade
/// gracefully rather than probing for `NoHistory`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    pub quotes: bool,
    pub instruments: bool,
    pub history: bool,
    /// True when the source stamps its data. False means every `as_of` this
    /// adapter emits is imputed.
    pub timestamps: bool,
}

#[async_trait]
pub trait Adapter: Send + Sync + 'static {
    /// Stable id recorded in every `Provenance` this adapter produces.
    fn id(&self) -> &'static str;

    fn exchanges(&self) -> &'static [ExchangeCode];

    fn capabilities(&self) -> Capabilities;

    /// Human-readable licensing note, surfaced in the API's source listing so
    /// consumers can see what they are actually being served.
    fn terms(&self) -> &'static str;

    async fn list_instruments(
        &self,
        exchange: ExchangeCode,
    ) -> Result<Vec<Instrument>, AdapterError>;

    /// Latest quotes. An empty `symbols` slice means the whole board.
    async fn quotes(
        &self,
        exchange: ExchangeCode,
        symbols: &[String],
    ) -> Result<Vec<Quote>, AdapterError>;

    async fn history(
        &self,
        exchange: ExchangeCode,
        _symbol: &str,
    ) -> Result<Vec<Bar>, AdapterError> {
        let _ = exchange;
        Err(AdapterError::NoHistory {
            source_id: self.id(),
        })
    }
}
