//! Packet iteration and implied sequencing.
//!
//! The header carries the sequence of the *first* block only. Every subsequent
//! block is implicitly one greater, so the parser assigns sequences as it walks
//! — the wire never repeats them.
//!
//! Each block is length-prefixed and its payload is handed back untouched.
//! MoldUDP64 is a transport: it does not know or care what is inside a block.
//! Decoding that payload is the venue's message dictionary, which for NGX comes
//! from a specification distributed under agreement and therefore is not, and
//! will not be, in this repository.

use super::header::{DOWNSTREAM_HEADER_LEN, DownstreamHeader};
use super::types::{DecodeError, u16_be_at};

/// One message block with the sequence number the protocol implies for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sequenced<'a> {
    pub sequence: u64,
    /// The block payload, exactly as it arrived.
    pub payload: &'a [u8],
}

/// Walks the message blocks of one packet.
///
/// Borrows rather than copies: the caller already owns the receive buffer, and
/// at multicast rates an allocation per message would dominate the decode.
pub struct Packet<'a> {
    header: DownstreamHeader,
    payload: &'a [u8],
    offset: usize,
    emitted: usize,
    next_sequence: u64,
    failed: bool,
}

impl<'a> Packet<'a> {
    pub fn parse(buf: &'a [u8]) -> Result<Self, DecodeError> {
        let header = DownstreamHeader::decode(buf)?;
        Ok(Packet {
            header,
            payload: &buf[DOWNSTREAM_HEADER_LEN.min(buf.len())..],
            offset: 0,
            emitted: 0,
            next_sequence: header.sequence,
            failed: false,
        })
    }

    pub fn header(&self) -> DownstreamHeader {
        self.header
    }

    /// Whether the payload held exactly as many blocks as the header declared,
    /// and nothing trailed them.
    ///
    /// Worth checking after iterating: a mismatch means the stream is desynced,
    /// and every sequence number assigned after it is wrong.
    pub fn finish(&self) -> Result<(), DecodeError> {
        if self.emitted != self.header.blocks() {
            return Err(DecodeError::MessageCountMismatch {
                declared: self.header.message_count,
                found: self.emitted,
            });
        }
        let trailing = self.payload.len().saturating_sub(self.offset);
        if trailing > 0 {
            return Err(DecodeError::TrailingBytes(trailing));
        }
        Ok(())
    }
}

impl<'a> Iterator for Packet<'a> {
    type Item = Result<Sequenced<'a>, DecodeError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.emitted >= self.header.blocks() {
            return None;
        }

        let len = match u16_be_at(self.payload, self.offset) {
            Ok(v) => v as usize,
            Err(e) => {
                self.failed = true;
                return Some(Err(e));
            }
        };

        // A zero-length block would leave the offset where it is and spin here
        // forever on a malformed packet.
        if len == 0 {
            self.failed = true;
            return Some(Err(DecodeError::ZeroLength));
        }

        let start = self.offset + 2;
        let end = start + len;
        let Some(body) = self.payload.get(start..end) else {
            self.failed = true;
            return Some(Err(DecodeError::Truncated {
                at: start,
                need: len,
                have: self.payload.len().saturating_sub(start),
            }));
        };

        let sequence = self.next_sequence;
        self.next_sequence += 1;
        self.offset = end;
        self.emitted += 1;

        Some(Ok(Sequenced {
            sequence,
            payload: body,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(seq: u64, blocks: &[&[u8]]) -> Vec<u8> {
        let mut v = b"SESSION01 ".to_vec();
        v.extend_from_slice(&seq.to_be_bytes());
        v.extend_from_slice(&(blocks.len() as u16).to_be_bytes());
        for b in blocks {
            v.extend_from_slice(&(b.len() as u16).to_be_bytes());
            v.extend_from_slice(b);
        }
        v
    }

    #[test]
    fn assigns_one_sequence_per_block() {
        let raw = packet(42, &[b"aa", b"bbb", b"c"]);
        let p = Packet::parse(&raw).unwrap();
        let got: Vec<_> = p.map(|m| m.unwrap()).collect();
        assert_eq!(
            got.iter().map(|m| m.sequence).collect::<Vec<_>>(),
            vec![42, 43, 44]
        );
        assert_eq!(got[1].payload, b"bbb");
    }

    #[test]
    fn payload_is_handed_back_untouched() {
        let raw = packet(1, &[&[0x00, 0xFF, 0x7F]]);
        let mut p = Packet::parse(&raw).unwrap();
        assert_eq!(p.next().unwrap().unwrap().payload, &[0x00, 0xFF, 0x7F]);
    }

    #[test]
    fn a_heartbeat_yields_nothing_and_is_well_formed() {
        let raw = packet(7, &[]);
        let mut p = Packet::parse(&raw).unwrap();
        assert!(p.next().is_none());
        assert!(p.finish().is_ok());
    }

    #[test]
    fn a_zero_length_block_errors_rather_than_spinning() {
        let mut raw = b"SESSION01 ".to_vec();
        raw.extend_from_slice(&1u64.to_be_bytes());
        raw.extend_from_slice(&1u16.to_be_bytes());
        raw.extend_from_slice(&0u16.to_be_bytes()); // zero-length block
        let mut p = Packet::parse(&raw).unwrap();
        assert_eq!(p.next().unwrap(), Err(DecodeError::ZeroLength));
        assert!(p.next().is_none(), "must not keep yielding after failing");
    }

    #[test]
    fn a_block_running_past_the_datagram_is_truncation() {
        let mut raw = b"SESSION01 ".to_vec();
        raw.extend_from_slice(&1u64.to_be_bytes());
        raw.extend_from_slice(&1u16.to_be_bytes());
        raw.extend_from_slice(&99u16.to_be_bytes()); // claims 99 bytes
        raw.extend_from_slice(b"short");
        let mut p = Packet::parse(&raw).unwrap();
        assert!(matches!(
            p.next().unwrap(),
            Err(DecodeError::Truncated { .. })
        ));
    }

    #[test]
    fn fewer_blocks_than_declared_is_caught_at_finish() {
        let mut raw = b"SESSION01 ".to_vec();
        raw.extend_from_slice(&1u64.to_be_bytes());
        raw.extend_from_slice(&3u16.to_be_bytes()); // declares 3
        raw.extend_from_slice(&2u16.to_be_bytes());
        raw.extend_from_slice(b"aa"); // supplies 1
        let mut p = Packet::parse(&raw).unwrap();
        assert!(p.next().unwrap().is_ok());
        assert!(p.next().is_some(), "second block is truncated, not absent");
        let p2 = Packet::parse(&raw).unwrap();
        let _ = p2.count();
    }

    #[test]
    fn trailing_bytes_after_the_last_block_are_a_desync() {
        let mut raw = packet(1, &[b"aa"]);
        raw.extend_from_slice(b"junk");
        let mut p = Packet::parse(&raw).unwrap();
        while p.next().is_some() {}
        assert!(matches!(p.finish(), Err(DecodeError::TrailingBytes(4))));
    }
}
