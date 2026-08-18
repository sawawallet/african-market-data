//! MITCH primitive types.
//!
//! Everything is little-endian. Every read is bounds-checked against the slice;
//! there is no `unsafe` in this codec. A market data decoder parses hostile
//! binary off the network, and the performance argument for skipping bounds
//! checks does not survive contact with one malformed packet.

use amd_core::{Currency, Price};

/// Implied decimal places on a MITCH `Price` field.
///
/// Fixed by the protocol. [`amd_core::DEFAULT_SCALE`] is 8 for exactly this
/// reason, so exchange-native prices land without rescaling and therefore
/// without rounding at ingestion.
pub const PRICE_SCALE: u8 = 8;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("truncated: need {need} bytes at offset {at}, have {have}")]
    Truncated { at: usize, need: usize, have: usize },
    #[error("unknown message type {0:#04x}")]
    UnknownMessageType(u8),
    #[error("message type {ty:#04x} declares length {declared}, expected {expected}")]
    BadLength {
        ty: u8,
        declared: usize,
        expected: usize,
    },
    #[error("declared unit length {declared} exceeds packet size {actual}")]
    BadUnitLength { declared: usize, actual: usize },
    #[error("unit header declares {declared} messages, payload held {found}")]
    MessageCountMismatch { declared: u8, found: usize },
    #[error("field {field} holds {value:#04x}, which is not a valid {expected}")]
    BadEnum {
        field: &'static str,
        value: u8,
        expected: &'static str,
    },
    #[error("message declares zero length, which would not advance the parser")]
    ZeroLength,
}

type R<T> = Result<T, DecodeError>;

#[inline]
fn need(buf: &[u8], at: usize, n: usize) -> R<&[u8]> {
    buf.get(at..at + n).ok_or(DecodeError::Truncated {
        at,
        need: n,
        have: buf.len().saturating_sub(at),
    })
}

#[inline]
pub fn u8_at(buf: &[u8], at: usize) -> R<u8> {
    Ok(need(buf, at, 1)?[0])
}

#[inline]
pub fn u16_at(buf: &[u8], at: usize) -> R<u16> {
    Ok(u16::from_le_bytes(
        need(buf, at, 2)?.try_into().expect("length checked"),
    ))
}

#[inline]
pub fn u32_at(buf: &[u8], at: usize) -> R<u32> {
    Ok(u32::from_le_bytes(
        need(buf, at, 4)?.try_into().expect("length checked"),
    ))
}

#[inline]
pub fn u64_at(buf: &[u8], at: usize) -> R<u64> {
    Ok(u64::from_le_bytes(
        need(buf, at, 8)?.try_into().expect("length checked"),
    ))
}

/// A MITCH `Price`: signed little-endian `i64` with eight implied decimals.
///
/// Widened to `i128` on the way into [`Price`] so later rescaling cannot
/// overflow, but the value on the wire is preserved exactly.
#[inline]
pub fn price_at(buf: &[u8], at: usize, currency: Currency) -> R<Price> {
    let raw = i64::from_le_bytes(need(buf, at, 8)?.try_into().expect("length checked"));
    Ok(Price {
        minor: raw as i128,
        scale: PRICE_SCALE,
        currency,
    })
}

/// An `Alpha` field: ASCII, left-justified, space-padded on the right.
#[inline]
pub fn alpha_at(buf: &[u8], at: usize, len: usize) -> R<&str> {
    let raw = need(buf, at, len)?;
    let end = raw.iter().rposition(|b| *b != b' ').map_or(0, |i| i + 1);
    // Non-ASCII in an Alpha field means a desynced stream, not exotic text.
    std::str::from_utf8(&raw[..end]).map_err(|_| DecodeError::BadEnum {
        field: "Alpha",
        value: 0,
        expected: "ASCII",
    })
}

/// Order side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn from_byte(b: u8) -> R<Self> {
        match b {
            b'B' => Ok(Side::Buy),
            b'S' => Ok(Side::Sell),
            v => Err(DecodeError::BadEnum {
                field: "Side",
                value: v,
                expected: "B or S",
            }),
        }
    }
}

/// Start or end of the trading day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemEventCode {
    StartOfDay,
    EndOfDay,
}

impl SystemEventCode {
    pub fn from_byte(b: u8) -> R<Self> {
        match b {
            b'O' => Ok(SystemEventCode::StartOfDay),
            b'C' => Ok(SystemEventCode::EndOfDay),
            v => Err(DecodeError::BadEnum {
                field: "Event Code",
                value: v,
                expected: "O or C",
            }),
        }
    }
}

/// A `Bit Field`: one byte holding up to eight flags, bit 0 least significant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flags(pub u8);

impl Flags {
    #[inline]
    pub fn bit(self, n: u8) -> bool {
        debug_assert!(n < 8, "bit index out of range");
        self.0 & (1 << n) != 0
    }
}
