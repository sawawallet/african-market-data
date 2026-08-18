//! Where a datum came from and how much to trust its freshness.
//!
//! Provenance is attached at ingestion and travels with the data all the way to
//! egress. Two reasons, both learned the hard way:
//!
//! 1. On thin African venues a stale price and a flat price look identical. A
//!    consumer that cannot see `as_of` will read a two-hour-old print as "the
//!    market is quiet today".
//! 2. Licensing is enforced per delay class. Carrying [`DelayClass`] on the
//!    datum lets the API boundary check entitlements per consumer, instead of
//!    discovering at audit time that a free tier was served licensed real-time.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// How far behind the market a datum is *as licensed*, not as measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelayClass {
    /// Direct from an exchange feed under a real-time entitlement.
    Realtime,
    /// Contractually delayed, typically 15 or 30 minutes.
    Delayed,
    /// Official end-of-day close.
    Eod,
    /// The source makes no claim. Untrusted for anything time-sensitive.
    Unknown,
}

impl DelayClass {
    /// Whether a consumer entitled to `self` may also be served `other`.
    ///
    /// Ordering is deliberate: a real-time entitlement subsumes delayed and
    /// EOD, but never the reverse. `Unknown` satisfies nothing but itself,
    /// because we cannot prove what it is.
    pub fn permits(self, other: DelayClass) -> bool {
        use DelayClass::*;
        match (self, other) {
            (Unknown, Unknown) => true,
            (Unknown, _) | (_, Unknown) => false,
            (Realtime, _) => true,
            (Delayed, Delayed | Eod) => true,
            (Eod, Eod) => true,
            _ => false,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            DelayClass::Realtime => "realtime",
            DelayClass::Delayed => "delayed",
            DelayClass::Eod => "eod",
            DelayClass::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Adapter or feed id, e.g. `"kwayisi"`, `"jse-mitch"`.
    pub source: String,
    /// Exchange timestamp where the source publishes one, else the receive time.
    #[serde(with = "time::serde::rfc3339")]
    pub as_of: OffsetDateTime,
    /// When this process received it. `as_of - received_at` is the feed lag,
    /// and it is the metric that tells you a feed is degrading before anyone
    /// complains.
    #[serde(with = "time::serde::rfc3339")]
    pub received_at: OffsetDateTime,
    pub delay: DelayClass,
    /// True when the source publishes no usable timestamp and `as_of` is the
    /// receive time standing in for it. Never chart these as if they were
    /// prints — you would be plotting your own polling schedule.
    pub as_of_imputed: bool,
    /// Sequence number from the venue feed, where the protocol provides one.
    /// MITCH and ITCH both do; REST sources do not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sequence: Option<u64>,
    /// True when this datum was reconstructed through a recovery path rather
    /// than received live. MITCH snapshot recovery does not preserve original
    /// timestamps, so microstructure analytics must exclude these.
    #[serde(default)]
    pub recovered: bool,
}

impl Provenance {
    /// Provenance for a source that publishes no timestamp of its own.
    pub fn imputed(source: impl Into<String>, at: OffsetDateTime, delay: DelayClass) -> Self {
        Provenance {
            source: source.into(),
            as_of: at,
            received_at: at,
            delay,
            as_of_imputed: true,
            sequence: None,
            recovered: false,
        }
    }

    /// Provenance for a source that stamps its own data.
    pub fn stamped(
        source: impl Into<String>,
        as_of: OffsetDateTime,
        received_at: OffsetDateTime,
        delay: DelayClass,
    ) -> Self {
        Provenance {
            source: source.into(),
            as_of,
            received_at,
            delay,
            as_of_imputed: false,
            sequence: None,
            recovered: false,
        }
    }

    pub fn with_sequence(mut self, sequence: u64) -> Self {
        self.sequence = Some(sequence);
        self
    }

    pub fn recovered(mut self) -> Self {
        self.recovered = true;
        self
    }

    /// Seconds between the exchange stamp and our receipt. Negative values are
    /// clamped to zero — they mean clock skew, not a datum from the future.
    pub fn lag_seconds(&self) -> i64 {
        (self.received_at - self.as_of).whole_seconds().max(0)
    }
}
