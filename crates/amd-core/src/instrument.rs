//! Instruments and the symbology that ties them together.
//!
//! Symbol resolution across sources is the unglamorous hard part of this
//! system. A vendor's `MTNGH`, the venue's own ticker and a global vendor's
//! identifier for the same instrument will not match, and there is no ISIN
//! discipline across African exchanges. The mapping is therefore data, not
//! code — see `amd-store` for where it is persisted.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::money::Currency;

/// Venue identifier. Short codes rather than MICs because several of these
/// venues have no MIC in common use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum ExchangeCode {
    Ngx,
    Jse,
    Nsx,
    Gse,
    Nse,
    Egx,
    Brvm,
    Cse,
    Sem,
    Bse,
    Luse,
    Dse,
    Use,
    Zse,
    Mse,
    Rse,
    Bvmt,
}

impl ExchangeCode {
    pub const ALL: [ExchangeCode; 17] = {
        use ExchangeCode::*;
        [
            Ngx, Jse, Nsx, Gse, Nse, Egx, Brvm, Cse, Sem, Bse, Luse, Dse, Use, Zse, Mse, Rse, Bvmt,
        ]
    };

    pub fn as_str(self) -> &'static str {
        use ExchangeCode::*;
        match self {
            Ngx => "NGX",
            Jse => "JSE",
            Nsx => "NSX",
            Gse => "GSE",
            Nse => "NSE",
            Egx => "EGX",
            Brvm => "BRVM",
            Cse => "CSE",
            Sem => "SEM",
            Bse => "BSE",
            Luse => "LUSE",
            Dse => "DSE",
            Use => "USE",
            Zse => "ZSE",
            Mse => "MSE",
            Rse => "RSE",
            Bvmt => "BVMT",
        }
    }
}

impl fmt::Display for ExchangeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("unknown exchange code: {0}")]
pub struct UnknownExchange(pub String);

impl FromStr for ExchangeCode {
    type Err = UnknownExchange;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.trim().to_ascii_uppercase();
        ExchangeCode::ALL
            .into_iter()
            .find(|c| c.as_str() == upper)
            .ok_or_else(|| UnknownExchange(s.to_owned()))
    }
}

/// Stable internal identity for an instrument: `EXCHANGE:SYMBOL`.
///
/// Deliberately not the ISIN. ISIN coverage across these venues is patchy and
/// occasionally wrong, so it is carried as an attribute rather than used as
/// the key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstrumentId {
    pub exchange: ExchangeCode,
    pub symbol: String,
}

impl InstrumentId {
    pub fn new(exchange: ExchangeCode, symbol: impl AsRef<str>) -> Self {
        InstrumentId {
            exchange,
            symbol: symbol.as_ref().trim().to_ascii_uppercase(),
        }
    }
}

impl fmt::Display for InstrumentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.exchange, self.symbol)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseInstrumentId {
    #[error("expected EXCHANGE:SYMBOL, got {0:?}")]
    Malformed(String),
    #[error(transparent)]
    Exchange(#[from] UnknownExchange),
}

impl FromStr for InstrumentId {
    type Err = ParseInstrumentId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (ex, sym) = s
            .split_once(':')
            .ok_or_else(|| ParseInstrumentId::Malformed(s.to_owned()))?;
        if sym.is_empty() {
            return Err(ParseInstrumentId::Malformed(s.to_owned()));
        }
        Ok(InstrumentId::new(ex.parse()?, sym))
    }
}

impl TryFrom<String> for InstrumentId {
    type Error = ParseInstrumentId;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<InstrumentId> for String {
    fn from(id: InstrumentId) -> String {
        id.to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Instrument {
    pub id: InstrumentId,
    pub currency: Currency,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shares_outstanding: Option<u64>,
}

impl Instrument {
    pub fn new(exchange: ExchangeCode, symbol: impl AsRef<str>, currency: Currency) -> Self {
        Instrument {
            id: InstrumentId::new(exchange, symbol),
            currency,
            name: None,
            isin: None,
            sector: None,
            shares_outstanding: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_sector(mut self, sector: impl Into<String>) -> Self {
        self.sector = Some(sector.into());
        self
    }

    pub fn with_shares(mut self, shares: u64) -> Self {
        self.shares_outstanding = Some(shares);
        self
    }

    pub fn exchange(&self) -> ExchangeCode {
        self.id.exchange
    }

    pub fn symbol(&self) -> &str {
        &self.id.symbol
    }
}
