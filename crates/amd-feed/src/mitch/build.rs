//! Synthetic packet construction, for tests and for replaying captures.
//!
//! Encoding is not needed to consume a feed — a client never sends application
//! messages. It exists so the decoder can be exercised without a JSE
//! connection, which is the difference between a codec that is tested and one
//! that is merely written. It is also what a capture replayer needs in order to
//! inject loss, reordering and duplication into a known-good stream.
//!
//! Kept in the shipped crate rather than under `#[cfg(test)]` so downstream
//! operators can build their own conformance fixtures.

use amd_core::Price;

use super::header::UNIT_HEADER_LEN;
use super::message::msg_type;
use super::types::{Flags, PRICE_SCALE, Side};

/// Accumulates payload messages, then frames them under a unit header.
#[derive(Debug, Default, Clone)]
pub struct PacketBuilder {
    payload: Vec<u8>,
    count: u8,
    market_data_group: u8,
}

fn price_bytes(p: Price) -> [u8; 8] {
    debug_assert_eq!(p.scale, PRICE_SCALE, "price must already be at MITCH scale");
    (p.minor as i64).to_le_bytes()
}

impl PacketBuilder {
    pub fn new(market_data_group: u8) -> Self {
        PacketBuilder {
            payload: Vec::new(),
            count: 0,
            market_data_group,
        }
    }

    fn push(&mut self, mut body: Vec<u8>) -> &mut Self {
        // Every message declares its own length, including the length byte.
        let len = body.len() + 1;
        assert!(
            len <= u8::MAX as usize,
            "message longer than a u8 length field"
        );
        let mut framed = Vec::with_capacity(len);
        framed.push(len as u8);
        framed.append(&mut body);
        self.payload.extend_from_slice(&framed);
        self.count += 1;
        self
    }

    pub fn time(&mut self, seconds: u32) -> &mut Self {
        let mut b = vec![msg_type::TIME];
        b.extend_from_slice(&seconds.to_le_bytes());
        self.push(b)
    }

    pub fn system_event(&mut self, nanos: u32, code: u8) -> &mut Self {
        let mut b = vec![msg_type::SYSTEM_EVENT];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.push(code);
        self.push(b)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_order(
        &mut self,
        nanos: u32,
        order_id: u64,
        side: Side,
        quantity: u32,
        instrument_id: u32,
        price: Price,
        flags: Flags,
    ) -> &mut Self {
        let mut b = vec![msg_type::ADD_ORDER];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&order_id.to_le_bytes());
        b.push(match side {
            Side::Buy => b'B',
            Side::Sell => b'S',
        });
        b.extend_from_slice(&quantity.to_le_bytes());
        b.extend_from_slice(&instrument_id.to_le_bytes());
        b.extend_from_slice(&[0, 0]); // reserved at offsets 23, 24
        b.extend_from_slice(&price_bytes(price));
        b.push(flags.0);
        b.extend_from_slice(&[b' '; 10]); // reserved Alpha at offset 34
        self.push(b)
    }

    pub fn order_deleted(
        &mut self,
        nanos: u32,
        order_id: u64,
        flags: Flags,
        instrument_id: u32,
    ) -> &mut Self {
        let mut b = vec![msg_type::ORDER_DELETED];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&order_id.to_le_bytes());
        b.push(flags.0);
        b.extend_from_slice(&instrument_id.to_le_bytes());
        self.push(b)
    }

    pub fn order_modified(
        &mut self,
        nanos: u32,
        order_id: u64,
        new_quantity: u32,
        new_price: Price,
        flags: Flags,
    ) -> &mut Self {
        let mut b = vec![msg_type::ORDER_MODIFIED];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&order_id.to_le_bytes());
        b.extend_from_slice(&new_quantity.to_le_bytes());
        b.extend_from_slice(&price_bytes(new_price));
        b.push(flags.0);
        self.push(b)
    }

    pub fn order_executed(
        &mut self,
        nanos: u32,
        order_id: u64,
        executed_quantity: u32,
        trade_match_id: u64,
    ) -> &mut Self {
        let mut b = vec![msg_type::ORDER_EXECUTED];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&order_id.to_le_bytes());
        b.extend_from_slice(&executed_quantity.to_le_bytes());
        b.extend_from_slice(&trade_match_id.to_le_bytes());
        self.push(b)
    }

    pub fn order_book_clear(&mut self, nanos: u32, instrument_id: u32, flags: Flags) -> &mut Self {
        let mut b = vec![msg_type::ORDER_BOOK_CLEAR];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&instrument_id.to_le_bytes());
        b.extend_from_slice(&[0, 0]); // reserved at offsets 10, 11
        b.push(flags.0);
        self.push(b)
    }

    pub fn trade(
        &mut self,
        nanos: u32,
        executed_quantity: u32,
        instrument_id: u32,
        price: Price,
        trade_match_id: u64,
    ) -> &mut Self {
        let mut b = vec![msg_type::TRADE];
        b.extend_from_slice(&nanos.to_le_bytes());
        b.extend_from_slice(&executed_quantity.to_le_bytes());
        b.extend_from_slice(&instrument_id.to_le_bytes());
        b.extend_from_slice(&[0, 0]); // reserved at offsets 14, 15
        b.extend_from_slice(&price_bytes(price));
        b.extend_from_slice(&trade_match_id.to_le_bytes());
        self.push(b)
    }

    /// Frame the accumulated messages under a unit header.
    pub fn finish(&self, sequence: u32) -> Vec<u8> {
        let total = UNIT_HEADER_LEN + self.payload.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&(total as u16).to_le_bytes());
        out.push(self.count);
        out.push(self.market_data_group);
        out.extend_from_slice(&sequence.to_le_bytes());
        out.extend_from_slice(&self.payload);
        out
    }

    /// A heartbeat: header only, no payload, does not advance the sequence.
    pub fn heartbeat(market_data_group: u8, sequence: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(UNIT_HEADER_LEN);
        out.extend_from_slice(&(UNIT_HEADER_LEN as u16).to_le_bytes());
        out.push(0);
        out.push(market_data_group);
        out.extend_from_slice(&sequence.to_le_bytes());
        out
    }

    pub fn message_count(&self) -> u8 {
        self.count
    }
}
