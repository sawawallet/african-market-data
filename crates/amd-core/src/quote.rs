//! Point-in-time observations and session bars.

use serde::{Deserialize, Serialize};

use crate::instrument::InstrumentId;
use crate::money::{MoneyError, Price};
use crate::provenance::Provenance;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub instrument: InstrumentId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_close: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bid: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ask: Option<Price>,
    /// Shares traded in the session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u64>,
    /// Number of trades, where the venue publishes it. NGX calls these "deals".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trades: Option<u32>,
    pub provenance: Provenance,
}

impl Quote {
    pub fn new(instrument: InstrumentId, provenance: Provenance) -> Self {
        Quote {
            instrument,
            last: None,
            previous_close: None,
            open: None,
            high: None,
            low: None,
            bid: None,
            ask: None,
            volume: None,
            trades: None,
            provenance,
        }
    }

    /// Session change in basis points, when both ends are known.
    pub fn change_bps(&self) -> Option<Result<i64, MoneyError>> {
        match (self.previous_close, self.last) {
            (Some(prev), Some(last)) => Some(prev.change_bps(last)),
            _ => None,
        }
    }

    /// Whether bid and ask are crossed — bid strictly above ask.
    ///
    /// A crossed book is either a decoder bug or a genuine venue condition
    /// during an auction, and both are worth catching before the quote is
    /// published rather than after a user reports it.
    pub fn is_crossed(&self) -> Option<bool> {
        match (self.bid, self.ask) {
            (Some(b), Some(a)) => b.cmp_value(a).ok().map(|o| o.is_gt()),
            _ => None,
        }
    }
}

/// One session's summary bar. `date` is the session date in the exchange's own
/// timezone, which is not always the UTC date of any contained timestamp.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bar {
    pub instrument: InstrumentId,
    /// Session date, exchange-local, as `YYYY-MM-DD`.
    pub date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub high: Option<Price>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub low: Option<Price>,
    pub close: Price,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<u64>,
    pub provenance: Provenance,
}
