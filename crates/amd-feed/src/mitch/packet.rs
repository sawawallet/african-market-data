//! Packet iteration and implied sequencing.
//!
//! The unit header carries the sequence of the *first* message only. Every
//! subsequent message in the packet is implicitly one greater, so the parser
//! assigns sequences as it walks — the wire never repeats them.

use amd_core::Currency;

use super::header::{UNIT_HEADER_LEN, UnitHeader};
use super::message::{Message, decode};
use super::types::DecodeError;

/// One message with the sequence number the protocol implies for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequenced<'a> {
    /// Zero for unsequenced traffic — administrative messages and anything on
    /// the Recovery channel.
    pub sequence: u32,
    pub message: Message<'a>,
}

/// Walks the payload of one packet.
///
/// Borrows the packet rather than copying: at full-depth multicast rates the
/// allocation per message would dominate the decode, and the caller already
/// owns the receive buffer.
pub struct Packet<'a> {
    header: UnitHeader,
    payload: &'a [u8],
    currency: Currency,
    offset: usize,
    emitted: usize,
    next_sequence: u32,
    failed: bool,
}

impl<'a> Packet<'a> {
    /// Parse the unit header and prepare to walk the payload.
    pub fn parse(buf: &'a [u8], currency: Currency) -> Result<Self, DecodeError> {
        let header = UnitHeader::decode(buf)?;
        let end = header.length as usize;
        Ok(Packet {
            header,
            payload: &buf[UNIT_HEADER_LEN..end],
            currency,
            offset: 0,
            emitted: 0,
            next_sequence: header.sequence,
            failed: false,
        })
    }

    pub fn header(&self) -> UnitHeader {
        self.header
    }

    /// Whether the payload held exactly as many messages as the header declared.
    ///
    /// Worth checking after iterating: a mismatch means the stream is desynced,
    /// and every sequence number assigned after it is wrong.
    pub fn verify_count(&self) -> Result<(), DecodeError> {
        if self.emitted == self.header.message_count as usize {
            Ok(())
        } else {
            Err(DecodeError::MessageCountMismatch {
                declared: self.header.message_count,
                found: self.emitted,
            })
        }
    }
}

impl<'a> Iterator for Packet<'a> {
    type Item = Result<Sequenced<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.offset >= self.payload.len() {
            return None;
        }
        match decode(&self.payload[self.offset..], self.currency) {
            Ok((message, len)) => {
                self.offset += len;
                self.emitted += 1;
                let sequence = if self.header.is_unsequenced() {
                    0
                } else {
                    self.next_sequence
                };
                // Unsequenced traffic must not advance the counter, or the next
                // real packet reads as a gap.
                if !self.header.is_unsequenced() {
                    self.next_sequence = self.next_sequence.wrapping_add(1);
                }
                Some(Ok(Sequenced { sequence, message }))
            }
            Err(e) => {
                // Stop at the first bad message. Continuing would emit sequence
                // numbers derived from a parser that has lost its place.
                self.failed = true;
                Some(Err(e))
            }
        }
    }
}
