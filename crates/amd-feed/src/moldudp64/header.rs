//! The downstream packet header.
//!
//! Twenty bytes prefixing every UDP datagram. One header per packet, always.
//!
//! ```text
//! offset  len  type      field
//!      0   10  Alpha     Session         — constant for the trading day
//!     10    8  UInt64BE  Sequence Number — of the FIRST message block
//!     18    2  UInt16BE  Message Count   — blocks following
//!     20    -  -         Message blocks
//! ```
//!
//! Two differences from the MITCH unit header are worth stating plainly,
//! because both change what the sequencer must do:
//!
//! - The sequence is a `u64`, not a `u32`. At any plausible message rate it
//!   will not wrap, so wrapping arithmetic is not needed and its absence is
//!   deliberate — wrapping here would hide a desync rather than surface it.
//! - A restart is signalled by the **session id changing**, not by the sequence
//!   returning to 1. A handler that watches only the sequence will sit happily
//!   on a new session applying messages against a stale book.

use super::types::{DecodeError, Session, u16_be_at, u64_be_at};

pub const DOWNSTREAM_HEADER_LEN: usize = 20;

/// A message count of `0xFFFF` ends the session. It is a sentinel, not a count:
/// no blocks follow it.
pub const END_OF_SESSION: u16 = 0xFFFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownstreamHeader {
    pub session: Session,
    /// Sequence number of the *first* message block. Subsequent blocks in the
    /// same packet are implicitly one greater each.
    pub sequence: u64,
    pub message_count: u16,
}

impl DownstreamHeader {
    pub fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        Ok(DownstreamHeader {
            session: Session::decode(buf, 0)?,
            sequence: u64_be_at(buf, 10)?,
            message_count: u16_be_at(buf, 18)?,
        })
    }

    /// Sequence number the *next* packet should carry.
    ///
    /// This identity is the whole gap detector, so it lives in exactly one
    /// place. Heartbeats and the end-of-session sentinel carry no blocks and
    /// must not advance it — treating `0xFFFF` as a count would jump the
    /// expected sequence by 65,535 and make every subsequent packet look like
    /// a catastrophic backwards gap.
    #[inline]
    pub fn next_expected(&self) -> u64 {
        if self.is_heartbeat() || self.is_end_of_session() {
            self.sequence
        } else {
            self.sequence + self.message_count as u64
        }
    }

    /// A heartbeat: no blocks, sent to keep the line warm during inactivity.
    #[inline]
    pub fn is_heartbeat(&self) -> bool {
        self.message_count == 0
    }

    #[inline]
    pub fn is_end_of_session(&self) -> bool {
        self.message_count == END_OF_SESSION
    }

    /// How many blocks actually follow the header.
    #[inline]
    pub fn blocks(&self) -> usize {
        if self.is_end_of_session() {
            0
        } else {
            self.message_count as usize
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(seq: u64, count: u16) -> Vec<u8> {
        let mut v = b"SESSION01 ".to_vec();
        v.extend_from_slice(&seq.to_be_bytes());
        v.extend_from_slice(&count.to_be_bytes());
        v
    }

    #[test]
    fn decodes_big_endian_not_little() {
        let h = DownstreamHeader::decode(&header(1, 2)).unwrap();
        assert_eq!(
            h.sequence, 1,
            "a little-endian read would give a huge number"
        );
        assert_eq!(h.message_count, 2);
        assert_eq!(h.session.as_str(), "SESSION01");
    }

    #[test]
    fn next_expected_advances_by_the_block_count() {
        let h = DownstreamHeader::decode(&header(100, 5)).unwrap();
        assert_eq!(h.next_expected(), 105);
    }

    #[test]
    fn a_heartbeat_does_not_advance_the_sequence() {
        let h = DownstreamHeader::decode(&header(100, 0)).unwrap();
        assert!(h.is_heartbeat());
        assert_eq!(h.next_expected(), 100);
    }

    #[test]
    fn end_of_session_is_a_sentinel_not_a_count_of_65535() {
        let h = DownstreamHeader::decode(&header(100, END_OF_SESSION)).unwrap();
        assert!(h.is_end_of_session());
        assert_eq!(h.blocks(), 0);
        assert_eq!(h.next_expected(), 100);
    }

    #[test]
    fn a_short_datagram_is_an_error_not_a_panic() {
        assert!(DownstreamHeader::decode(&[0u8; 19]).is_err());
    }
}
