//! ClickHouse tick archive.
//!
//! Writes go through a batched inserter rather than one row per statement:
//! ClickHouse is a columnar store and a per-row insert creates a part per row,
//! which degrades merge performance badly enough to notice within a day.

use amd_core::{Provenance, Quote};
use clickhouse::{Client, Row};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::StoreError;

/// Must match the `Enum8` definition in `deploy/clickhouse/001_schema.sql`.
pub fn delay_code(d: amd_core::DelayClass) -> i8 {
    use amd_core::DelayClass::*;
    match d {
        Realtime => 1,
        Delayed => 2,
        Eod => 3,
        Unknown => 4,
    }
}

pub fn delay_from_code(code: i8) -> Option<amd_core::DelayClass> {
    use amd_core::DelayClass::*;
    match code {
        1 => Some(Realtime),
        2 => Some(Delayed),
        3 => Some(Eod),
        4 => Some(Unknown),
        _ => None,
    }
}

/// One archived observation.
///
/// Prices land as the scaled integer plus its scale — byte-for-byte what the
/// exchange published. Reassembling a `Price` needs `price_scale` and
/// `currency`, both carried alongside.
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct QuoteRow {
    pub exchange: String,
    pub symbol: String,
    #[serde(with = "clickhouse::serde::time::datetime64::nanos")]
    pub as_of: OffsetDateTime,
    #[serde(with = "clickhouse::serde::time::datetime64::nanos")]
    pub received_at: OffsetDateTime,
    pub source: String,
    /// Enum8 on the wire, which RowBinary encodes as a single signed byte.
    /// Sending the label as a String desyncs the row stream and ClickHouse
    /// reports a nonsensical expected length several rows later, so the
    /// mapping is explicit here and pinned by `delay_class_codes_match_schema`.
    pub delay_class: i8,
    pub as_of_imputed: u8,
    pub sequence: Option<u64>,
    pub recovered: u8,
    pub currency: String,
    pub price_scale: u8,
    pub last: Option<i128>,
    pub previous_close: Option<i128>,
    pub open: Option<i128>,
    pub high: Option<i128>,
    pub low: Option<i128>,
    pub bid: Option<i128>,
    pub ask: Option<i128>,
    pub volume: Option<u64>,
    pub trades: Option<u32>,
}

impl QuoteRow {
    pub fn from_quote(q: &Quote) -> Result<Self, StoreError> {
        // Every price on a quote must share one scale and currency, else the
        // integers in this row are not comparable to each other.
        let reference = [
            q.last,
            q.previous_close,
            q.open,
            q.high,
            q.low,
            q.bid,
            q.ask,
        ]
        .into_iter()
        .flatten()
        .next()
        .ok_or_else(|| {
            StoreError::Invalid(format!("{} has no price fields to archive", q.instrument))
        })?;

        let mut scale = reference.scale;
        for p in [
            q.last,
            q.previous_close,
            q.open,
            q.high,
            q.low,
            q.bid,
            q.ask,
        ]
        .into_iter()
        .flatten()
        {
            if p.currency != reference.currency {
                return Err(StoreError::Invalid(format!(
                    "{} mixes {} and {} on one quote",
                    q.instrument, reference.currency, p.currency
                )));
            }
            scale = scale.max(p.scale);
        }

        let at = |p: Option<amd_core::Price>| -> Result<Option<i128>, StoreError> {
            p.map(|p| p.rescale(scale).map(|r| r.minor))
                .transpose()
                .map_err(|e| StoreError::Invalid(e.to_string()))
        };

        let Provenance {
            source,
            as_of,
            received_at,
            delay,
            as_of_imputed,
            sequence,
            recovered,
        } = &q.provenance;

        Ok(QuoteRow {
            exchange: q.instrument.exchange.to_string(),
            symbol: q.instrument.symbol.clone(),
            as_of: *as_of,
            received_at: *received_at,
            source: source.clone(),
            delay_class: delay_code(*delay),
            as_of_imputed: u8::from(*as_of_imputed),
            sequence: *sequence,
            recovered: u8::from(*recovered),
            currency: reference.currency.to_string(),
            price_scale: scale,
            last: at(q.last)?,
            previous_close: at(q.previous_close)?,
            open: at(q.open)?,
            high: at(q.high)?,
            low: at(q.low)?,
            bid: at(q.bid)?,
            ask: at(q.ask)?,
            volume: q.volume,
            trades: q.trades,
        })
    }
}

#[derive(Clone)]
pub struct ClickhouseStore {
    client: Client,
}

impl ClickhouseStore {
    pub fn new(url: &str, user: &str, password: &str, database: &str) -> Self {
        ClickhouseStore {
            client: Client::default()
                .with_url(url)
                .with_user(user)
                .with_password(password)
                .with_database(database),
        }
    }

    /// Insert a batch. Callers should accumulate rather than call per quote.
    pub async fn insert_quotes(&self, rows: &[QuoteRow]) -> Result<usize, StoreError> {
        if rows.is_empty() {
            return Ok(0);
        }
        let mut insert = self
            .client
            .insert("quotes")
            .map_err(|e| StoreError::Clickhouse(e.to_string()))?;
        for row in rows {
            insert
                .write(row)
                .await
                .map_err(|e| StoreError::Clickhouse(e.to_string()))?;
        }
        insert
            .end()
            .await
            .map_err(|e| StoreError::Clickhouse(e.to_string()))?;
        Ok(rows.len())
    }

    /// Most recent archived observation per symbol for a venue.
    ///
    /// The API serves latest quotes from a cache; this is the cold path used on
    /// cache miss and at startup.
    pub async fn latest_quotes(&self, exchange: &str) -> Result<Vec<QuoteRow>, StoreError> {
        self.client
            .query(
                "SELECT ?fields FROM quotes WHERE exchange = ? \
                 ORDER BY symbol, as_of DESC LIMIT 1 BY symbol",
            )
            .bind(exchange)
            .fetch_all::<QuoteRow>()
            .await
            .map_err(|e| StoreError::Clickhouse(e.to_string()))
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        self.client
            .query("SELECT 1")
            .execute()
            .await
            .map_err(|e| StoreError::Clickhouse(e.to_string()))
    }
}
