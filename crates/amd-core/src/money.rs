//! Fixed-point money. No floating point anywhere in the price path.
//!
//! Exchange feeds publish scaled integers — MITCH prices are signed `i64` with
//! eight implied decimal places — and holding that shape end to end is what
//! keeps sums, differences and comparisons exact. A float creeps in once and
//! reconciliation breaks silently months later.

use std::fmt;

use serde::{Deserialize, Serialize};

/// ISO 4217 alphabetic currency code, stored inline to avoid an allocation on
/// every price. Every African venue we target uses a 3-letter code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Currency([u8; 3]);

impl Currency {
    pub const NGN: Currency = Currency(*b"NGN");
    pub const ZAR: Currency = Currency(*b"ZAR");
    pub const GHS: Currency = Currency(*b"GHS");
    pub const KES: Currency = Currency(*b"KES");
    pub const EGP: Currency = Currency(*b"EGP");
    pub const USD: Currency = Currency(*b"USD");

    pub const fn new(code: [u8; 3]) -> Self {
        Currency(code)
    }

    pub fn as_str(&self) -> &str {
        // Constructors validate ASCII, so this cannot fail.
        std::str::from_utf8(&self.0).unwrap_or("???")
    }
}

impl std::str::FromStr for Currency {
    type Err = MoneyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let b = s.as_bytes();
        if b.len() != 3 || !b.iter().all(|c| c.is_ascii_alphabetic()) {
            return Err(MoneyError::BadCurrency(s.to_owned()));
        }
        Ok(Currency([
            b[0].to_ascii_uppercase(),
            b[1].to_ascii_uppercase(),
            b[2].to_ascii_uppercase(),
        ]))
    }
}

impl TryFrom<String> for Currency {
    type Error = MoneyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl From<Currency> for String {
    fn from(c: Currency) -> String {
        c.as_str().to_owned()
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Currency({})", self.as_str())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MoneyError {
    #[error("not a valid ISO 4217 code: {0}")]
    BadCurrency(String),
    #[error("scale must be in 0..=18, got {0}")]
    BadScale(u8),
    #[error("not a decimal number: {0:?}")]
    NotANumber(String),
    #[error(
        "{value} has {actual} decimals but scale is {scale}; raise the scale or round explicitly"
    )]
    PrecisionLoss {
        value: String,
        actual: usize,
        scale: u8,
    },
    #[error("currency mismatch: {0} vs {1}")]
    CurrencyMismatch(Currency, Currency),
    #[error("arithmetic overflow in fixed-point operation")]
    Overflow,
    #[error("cannot compute change from a zero base")]
    ZeroBase,
}

/// A price as a scaled integer.
///
/// `Price { minor: 3542, scale: 2 }` is 35.42. `i128` rather than `i64` so that
/// rescaling a MITCH 8-decimal price up to a common working scale cannot
/// overflow mid-calculation.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Price {
    pub minor: i128,
    pub scale: u8,
    pub currency: Currency,
}

/// Scale used across the pipeline unless a venue demands more.
///
/// Eight matches MITCH's implied decimals, so exchange-native prices land here
/// without rescaling and therefore without any rounding at ingestion.
pub const DEFAULT_SCALE: u8 = 8;

impl Price {
    pub fn new(minor: i128, scale: u8, currency: Currency) -> Result<Self, MoneyError> {
        if scale > 18 {
            return Err(MoneyError::BadScale(scale));
        }
        Ok(Price {
            minor,
            scale,
            currency,
        })
    }

    pub fn zero(currency: Currency) -> Self {
        Price {
            minor: 0,
            scale: 0,
            currency,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.minor == 0
    }

    /// Parse a decimal string exactly.
    ///
    /// Takes a string rather than an `f64` because `0.1 + 0.2 != 0.3` is the
    /// reason this module exists. Refuses to truncate: a source that sends more
    /// precision than `scale` is an error to surface, not to silently discard.
    pub fn parse(input: &str, currency: Currency, scale: u8) -> Result<Self, MoneyError> {
        if scale > 18 {
            return Err(MoneyError::BadScale(scale));
        }
        let trimmed: String = input
            .trim()
            .chars()
            .filter(|c| *c != ',' && *c != '_')
            .collect();
        let (neg, body) = match trimmed.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, trimmed.strip_prefix('+').unwrap_or(&trimmed)),
        };
        let (whole, frac) = match body.split_once('.') {
            Some((w, f)) => (w, f),
            None => (body, ""),
        };
        if whole.is_empty() && frac.is_empty() {
            return Err(MoneyError::NotANumber(input.to_owned()));
        }
        if !whole.bytes().all(|b| b.is_ascii_digit()) || !frac.bytes().all(|b| b.is_ascii_digit()) {
            return Err(MoneyError::NotANumber(input.to_owned()));
        }
        if frac.len() > scale as usize {
            return Err(MoneyError::PrecisionLoss {
                value: trimmed.clone(),
                actual: frac.len(),
                scale,
            });
        }
        let mut digits = String::with_capacity(whole.len() + scale as usize);
        digits.push_str(if whole.is_empty() { "0" } else { whole });
        digits.push_str(frac);
        for _ in frac.len()..scale as usize {
            digits.push('0');
        }
        let magnitude: i128 = digits.parse().map_err(|_| MoneyError::Overflow)?;
        Ok(Price {
            minor: if neg { -magnitude } else { magnitude },
            scale,
            currency,
        })
    }

    /// Convert a JSON number to a `Price`.
    ///
    /// Only for sources that publish prices as JSON numbers and give us no raw
    /// string to work from — the kwayisi GSE feed does exactly this. Uses the
    /// float's shortest round-trip representation, which is the most faithful
    /// reading available once a source has already put the value through a
    /// float. Prefer [`Price::parse`] wherever a string is on offer.
    pub fn from_f64(n: f64, currency: Currency, scale: u8) -> Result<Self, MoneyError> {
        if !n.is_finite() {
            return Err(MoneyError::NotANumber(n.to_string()));
        }
        let repr = format!("{n}");
        if repr.contains(['e', 'E']) {
            return Price::parse(&format!("{n:.*}", scale as usize), currency, scale);
        }
        let frac_len = repr.split_once('.').map_or(0, |(_, f)| f.len());
        if frac_len > scale as usize {
            Price::parse(&format!("{n:.*}", scale as usize), currency, scale)
        } else {
            Price::parse(&repr, currency, scale)
        }
    }

    /// Rescale, rounding half away from zero.
    pub fn rescale(self, scale: u8) -> Result<Self, MoneyError> {
        if scale > 18 {
            return Err(MoneyError::BadScale(scale));
        }
        if scale == self.scale {
            return Ok(self);
        }
        if scale > self.scale {
            let factor = 10i128
                .checked_pow((scale - self.scale) as u32)
                .ok_or(MoneyError::Overflow)?;
            let minor = self.minor.checked_mul(factor).ok_or(MoneyError::Overflow)?;
            return Ok(Price {
                minor,
                scale,
                currency: self.currency,
            });
        }
        let divisor = 10i128
            .checked_pow((self.scale - scale) as u32)
            .ok_or(MoneyError::Overflow)?;
        let q = self.minor / divisor; // truncates toward zero
        let rem = (self.minor % divisor).abs();
        let minor = if rem * 2 >= divisor {
            q + if self.minor < 0 { -1 } else { 1 }
        } else {
            q
        };
        Ok(Price {
            minor,
            scale,
            currency: self.currency,
        })
    }

    fn align(self, other: Self) -> Result<(i128, i128, u8), MoneyError> {
        if self.currency != other.currency {
            return Err(MoneyError::CurrencyMismatch(self.currency, other.currency));
        }
        let scale = self.scale.max(other.scale);
        Ok((
            self.rescale(scale)?.minor,
            other.rescale(scale)?.minor,
            scale,
        ))
    }

    /// Not `std::ops::Add`. Addition here is fallible in two ways that must
    /// surface as errors rather than panics or silent wrapping: a currency
    /// mismatch, and fixed-point overflow. `Add::add` returns `Self`, so it
    /// cannot express either.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Result<Self, MoneyError> {
        let (a, b, scale) = self.align(other)?;
        let minor = a.checked_add(b).ok_or(MoneyError::Overflow)?;
        Ok(Price {
            minor,
            scale,
            currency: self.currency,
        })
    }

    /// Fallible for the same reasons as [`Price::add`].
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, other: Self) -> Result<Self, MoneyError> {
        let (a, b, scale) = self.align(other)?;
        let minor = a.checked_sub(b).ok_or(MoneyError::Overflow)?;
        Ok(Price {
            minor,
            scale,
            currency: self.currency,
        })
    }

    pub fn cmp_value(self, other: Self) -> Result<std::cmp::Ordering, MoneyError> {
        let (a, b, _) = self.align(other)?;
        Ok(a.cmp(&b))
    }

    /// Change from `self` to `to`, in basis points. 250 bps is +2.50%.
    ///
    /// Basis points rather than a float percentage so the result stays exact
    /// and directly comparable across instruments.
    pub fn change_bps(self, to: Self) -> Result<i64, MoneyError> {
        if self.is_zero() {
            return Err(MoneyError::ZeroBase);
        }
        let (from, to_v, _) = self.align(to)?;
        let delta = to_v.checked_sub(from).ok_or(MoneyError::Overflow)?;
        let scaled = delta.checked_mul(10_000).ok_or(MoneyError::Overflow)?;
        i64::try_from(scaled / from).map_err(|_| MoneyError::Overflow)
    }

    /// Lossy conversion for display and charting only.
    pub fn to_f64(self) -> f64 {
        self.minor as f64 / 10f64.powi(self.scale as i32)
    }
}

impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let neg = self.minor < 0;
        let abs = self.minor.unsigned_abs().to_string();
        let s = if abs.len() <= self.scale as usize {
            format!("{}{}", "0".repeat(self.scale as usize + 1 - abs.len()), abs)
        } else {
            abs
        };
        let split = s.len() - self.scale as usize;
        let (whole, frac) = s.split_at(split);

        // Group thousands from the right.
        let mut grouped = String::with_capacity(whole.len() + whole.len() / 3);
        for (i, c) in whole.chars().enumerate() {
            if i > 0 && (whole.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(c);
        }

        if neg {
            f.write_str("-")?;
        }
        write!(f, "{} {}", self.currency, grouped)?;
        if self.scale > 0 {
            write!(f, ".{frac}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Price({self})")
    }
}
