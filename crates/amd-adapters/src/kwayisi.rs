//! Ghana Stock Exchange via the kwayisi GSE API.
//!
//! First adapter because it is the one African source that is unambiguously
//! free to use: no registration, no key, and the maintainer states it is
//! "completely free and unencumbered for everyone".
//!
//! Two honest limitations, both reflected in the provenance this adapter emits:
//!
//! - The API publishes **no timestamp**, so `as_of` is the receive time and
//!   `as_of_imputed` is true. Nothing in the payload says when a price printed.
//! - Freshness is not contractually specified and observed behaviour is close
//!   to end-of-day, so the delay class is `Unknown` rather than `Realtime`.
//!
//! Endpoints: <https://dev.kwayisi.org/apis/gse/>

use amd_core::{
    Bar, Currency, DEFAULT_SCALE, DelayClass, ExchangeCode, Instrument, InstrumentId, Price,
    Provenance, Quote,
};
use async_trait::async_trait;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::http::HttpClient;
use crate::{Adapter, AdapterError, Capabilities};

const ID: &str = "kwayisi";
const BASE: &str = "https://dev.kwayisi.org/apis/gse";
const EXCHANGES: &[ExchangeCode] = &[ExchangeCode::Gse];

/// A `/live` row. `name` is the ticker, not the company name, and `change` is
/// an absolute price move rather than a percentage.
#[derive(Debug, Deserialize)]
struct LiveRow {
    name: String,
    price: f64,
    #[serde(default)]
    change: f64,
    #[serde(default)]
    volume: i64,
}

#[derive(Debug, Deserialize)]
struct EquityRow {
    name: String,
}

#[derive(Debug, Deserialize)]
struct CompanyDetail {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    sector: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EquityDetail {
    name: String,
    #[serde(default)]
    shares: Option<f64>,
    #[serde(default)]
    company: Option<CompanyDetail>,
}

pub struct KwayisiAdapter {
    http: HttpClient,
}

impl KwayisiAdapter {
    pub fn new(http: HttpClient) -> Self {
        KwayisiAdapter { http }
    }

    fn check(exchange: ExchangeCode) -> Result<(), AdapterError> {
        if exchange != ExchangeCode::Gse {
            return Err(AdapterError::UnsupportedExchange {
                source_id: ID,
                exchange,
            });
        }
        Ok(())
    }

    async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T, AdapterError> {
        self.http
            .get_json::<T>(&format!("{BASE}{path}"))
            .await
            .map_err(|e| AdapterError::Request {
                source_id: ID,
                message: e.to_string(),
            })
    }

    /// Company detail for one ticker. Outside the `Adapter` trait because most
    /// sources have no equivalent.
    pub async fn instrument_detail(&self, symbol: &str) -> Result<Instrument, AdapterError> {
        let d: EquityDetail = self
            .get(&format!("/equities/{}", symbol.to_ascii_uppercase()))
            .await?;
        let mut inst = Instrument::new(ExchangeCode::Gse, &d.name, Currency::GHS);
        if let Some(c) = d.company {
            if let Some(n) = c.name {
                inst = inst.with_name(n);
            }
            if let Some(s) = c.sector {
                inst = inst.with_sector(s);
            }
        }
        // `shares` arrives as a JSON float. Values are whole share counts well
        // inside f64's exact-integer range, but guard anyway.
        if let Some(shares) = d.shares
            && shares.is_finite()
            && shares >= 0.0
            && shares <= u64::MAX as f64
        {
            inst = inst.with_shares(shares as u64);
        }
        Ok(inst)
    }
}

#[async_trait]
impl Adapter for KwayisiAdapter {
    fn id(&self) -> &'static str {
        ID
    }

    fn exchanges(&self) -> &'static [ExchangeCode] {
        EXCHANGES
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            quotes: true,
            instruments: true,
            history: false,
            timestamps: false,
        }
    }

    fn terms(&self) -> &'static str {
        "Free, no registration required. Publishes no timestamp, so freshness is imputed \
         and the delay class is unknown."
    }

    async fn list_instruments(
        &self,
        exchange: ExchangeCode,
    ) -> Result<Vec<Instrument>, AdapterError> {
        Self::check(exchange)?;
        let rows: Vec<EquityRow> = self.get("/equities").await?;
        Ok(rows
            .into_iter()
            .map(|r| Instrument::new(ExchangeCode::Gse, &r.name, Currency::GHS))
            .collect())
    }

    async fn quotes(
        &self,
        exchange: ExchangeCode,
        symbols: &[String],
    ) -> Result<Vec<Quote>, AdapterError> {
        Self::check(exchange)?;
        let rows: Vec<LiveRow> = self.get("/live").await?;
        let received_at = OffsetDateTime::now_utc();

        let wanted: Option<Vec<String>> = if symbols.is_empty() {
            None
        } else {
            Some(symbols.iter().map(|s| s.to_ascii_uppercase()).collect())
        };

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let symbol = row.name.to_ascii_uppercase();
            if let Some(w) = &wanted
                && !w.contains(&symbol)
            {
                continue;
            }

            let provenance = Provenance::imputed(ID, received_at, DelayClass::Unknown);
            let mut quote = Quote::new(InstrumentId::new(ExchangeCode::Gse, &symbol), provenance);

            let last = Price::from_f64(row.price, Currency::GHS, DEFAULT_SCALE).map_err(|e| {
                AdapterError::Malformed {
                    source_id: ID,
                    message: format!("{symbol} price: {e}"),
                }
            })?;
            quote.last = Some(last);

            // The feed gives absolute change, so previous close is exact rather
            // than inferred.
            let change =
                Price::from_f64(row.change, Currency::GHS, DEFAULT_SCALE).map_err(|e| {
                    AdapterError::Malformed {
                        source_id: ID,
                        message: format!("{symbol} change: {e}"),
                    }
                })?;
            quote.previous_close = last.sub(change).ok();

            quote.volume = u64::try_from(row.volume.max(0)).ok();
            out.push(quote);
        }
        Ok(out)
    }

    async fn history(
        &self,
        _exchange: ExchangeCode,
        _symbol: &str,
    ) -> Result<Vec<Bar>, AdapterError> {
        Err(AdapterError::NoHistory { source_id: ID })
    }
}
