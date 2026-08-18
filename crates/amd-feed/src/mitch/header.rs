//! The unit header.
//!
//! Eight bytes prefixing every UDP packet. One header per packet, always.
//!
//! ```text
//! offset  len  type    field
//!      0    2  UInt16  Length          — header + all payload messages
//!      2    1  UInt8   Message Count   — payload messages following
//!      3    1  Byte    Market Data Group
//!      4    4  UInt32  Sequence Number — of the FIRST payload message only
//!      8    -  -       Payload
//! ```

use super::types::{DecodeError, u8_at, u16_at, u32_at};

pub const UNIT_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnitHeader {
    /// Length of the whole block, header included.
    pub length: u16,
    pub message_count: u8,
    pub market_data_group: u8,
    /// Sequence number of the *first* payload message. Subsequent messages in
    /// the same packet are implicitly one greater each.
    pub sequence: u32,
}

impl UnitHeader {
    pub fn decode(buf: &[u8]) -> Result<Self, DecodeError> {
        let header = UnitHeader {
            length: u16_at(buf, 0)?,
            message_count: u8_at(buf, 2)?,
            market_data_group: u8_at(buf, 3)?,
            sequence: u32_at(buf, 4)?,
        };
        let declared = header.length as usize;
        if declared < UNIT_HEADER_LEN || declared > buf.len() {
            return Err(DecodeError::BadUnitLength {
                declared,
                actual: buf.len(),
            });
        }
        Ok(header)
    }

    /// Sequence number the *next* packet should carry.
    ///
    /// This one identity is the entire gap detector, so it lives in exactly one
    /// place. Getting it wrong makes every downstream recovery decision wrong,
    /// and the failure is silent — you build a book on a transition you never
    /// saw and it looks fine until it does not.
    #[inline]
    pub fn next_expected(&self) -> u32 {
        self.sequence.wrapping_add(self.message_count as u32)
    }

    /// A heartbeat: a header carrying no payload, sent to exercise the line
    /// during periods of inactivity. It does not advance the sequence.
    #[inline]
    pub fn is_heartbeat(&self) -> bool {
        self.message_count == 0
    }

    /// Administrative traffic and Recovery-channel application messages are
    /// unsequenced, and carry sequence zero.
    #[inline]
    pub fn is_unsequenced(&self) -> bool {
        self.sequence == 0
    }
}
