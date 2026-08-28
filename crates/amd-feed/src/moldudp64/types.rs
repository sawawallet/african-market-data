//! MoldUDP64 primitive types.
//!
//! Everything is big-endian — network byte order. That is the first thing that
//! differs from MITCH, where everything is little-endian, and it is the kind of
//! difference that produces plausible-looking garbage rather than an error if
//! you get it wrong.
//!
//! Every read is bounds-checked against the slice and there is no `unsafe`
//! here, for the same reason there is none in the MITCH codec: this parses
//! hostile binary off a network socket.

use std::fmt;

/// A MoldUDP64 session id: ten bytes, alphanumeric, left-justified and padded
/// on the right with spaces.
pub const SESSION_LEN: usize = 10;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    #[error("truncated: need {need} bytes at offset {at}, have {have}")]
    Truncated { at: usize, need: usize, have: usize },
    #[error("packet declares {declared} messages, payload held {found}")]
    MessageCountMismatch { declared: u16, found: usize },
    #[error("message block declares zero length, which would not advance the parser")]
    ZeroLength,
    #[error("{0} bytes trail the declared message blocks")]
    TrailingBytes(usize),
    #[error("session id is not printable ASCII")]
    BadSession,
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
pub fn u16_be_at(buf: &[u8], at: usize) -> R<u16> {
    Ok(u16::from_be_bytes(
        need(buf, at, 2)?.try_into().expect("length checked"),
    ))
}

#[inline]
pub fn u64_be_at(buf: &[u8], at: usize) -> R<u64> {
    Ok(u64::from_be_bytes(
        need(buf, at, 8)?.try_into().expect("length checked"),
    ))
}

/// The session a packet belongs to.
///
/// Worth a type rather than a `[u8; 10]`: the session id changing is how a
/// MoldUDP64 feed signals a restart, and that decision should not hinge on
/// somebody remembering to compare the right ten bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Session([u8; SESSION_LEN]);

impl Session {
    #[inline]
    pub fn decode(buf: &[u8], at: usize) -> R<Self> {
        let raw = need(buf, at, SESSION_LEN)?;
        if raw.iter().any(|b| !b.is_ascii_graphic() && *b != b' ') {
            return Err(DecodeError::BadSession);
        }
        Ok(Session(raw.try_into().expect("length checked")))
    }

    /// The id with its right-hand padding removed.
    pub fn as_str(&self) -> &str {
        let end = self.0.iter().rposition(|b| *b != b' ').map_or(0, |i| i + 1);
        std::str::from_utf8(&self.0[..end]).unwrap_or("")
    }

    pub const fn bytes(&self) -> &[u8; SESSION_LEN] {
        &self.0
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Session({:?})", self.as_str())
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
