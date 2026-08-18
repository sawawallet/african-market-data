//! Subject taxonomy.
//!
//! Shaped for the deployment topology rather than for tidiness. Venue feed
//! handlers run colocated — Lagos for NGX, Johannesburg for JSE and NSX — and
//! publish to a local NATS server configured as a **leaf node** of the central
//! cluster. The leaf forwards upstream and buffers through a WAN partition,
//! which is the behaviour that makes a Lagos-to-central link survivable without
//! any reconnect logic in the handler itself.
//!
//! Because a leaf forwards by subject, the venue must be high in the hierarchy:
//! that is what lets the Lagos leaf export only `amd.v1.NGX.>` and keeps a
//! partition in one region from affecting another.
//!
//! ```text
//! amd.v1.<EXCHANGE>.<KIND>.<SYMBOL>
//!
//! amd.v1.NGX.quote.MTNN
//! amd.v1.GSE.quote.MTNGH
//! amd.v1.JSE.bar.NPN
//! amd.v1.NGX.status          (venue-level, no symbol)
//! ```

use amd_core::{ExchangeCode, InstrumentId};

pub const PREFIX: &str = "amd.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Quote,
    Bar,
    Status,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Quote => "quote",
            Kind::Bar => "bar",
            Kind::Status => "status",
        }
    }
}

/// Subject for one instrument's events of a given kind.
pub fn for_instrument(id: &InstrumentId, kind: Kind) -> String {
    format!("{PREFIX}.{}.{}.{}", id.exchange, kind.as_str(), id.symbol)
}

/// Venue-level subject carrying no symbol.
pub fn for_venue(exchange: ExchangeCode, kind: Kind) -> String {
    format!("{PREFIX}.{exchange}.{}", kind.as_str())
}

/// Everything from one venue. This is what a leaf node exports.
pub fn venue_wildcard(exchange: ExchangeCode) -> String {
    format!("{PREFIX}.{exchange}.>")
}

/// One kind across one venue, e.g. every NGX quote.
pub fn kind_wildcard(exchange: ExchangeCode, kind: Kind) -> String {
    format!("{PREFIX}.{exchange}.{}.*", kind.as_str())
}

/// Every event on the bus. Used by the archiver, and by nothing else.
pub fn all() -> String {
    format!("{PREFIX}.>")
}
