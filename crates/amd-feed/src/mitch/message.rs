//! Application messages.
//!
//! Field offsets are taken from the LSE MIT303 Level 2 (MITCH) specification.
//! Each message carries its own `Length` byte at offset 0 — including that byte
//! — and the parser advances by that length rather than by a table of sizes,
//! because venues extend messages with trailing reserved fields and a decoder
//! that assumes a fixed width desynchronises the moment one does.
//!
//! Only the book-building set is decoded here. Symbol Directory, Symbol Status,
//! auction and statistics messages are recognised and skipped rather than
//! silently dropped, so an unhandled type is visible instead of invisible.

use amd_core::{Currency, Price};

use super::types::{
    DecodeError, Flags, Side, SystemEventCode, alpha_at, price_at, u8_at, u32_at, u64_at,
};

pub mod msg_type {
    pub const TIME: u8 = 0x54;
    pub const SYSTEM_EVENT: u8 = 0x53;
    pub const SYMBOL_DIRECTORY: u8 = 0x52;
    pub const SYMBOL_STATUS: u8 = 0x48;
    pub const ADD_ORDER: u8 = 0x41;
    pub const ADD_ATTRIBUTED_ORDER: u8 = 0x46;
    pub const ORDER_DELETED: u8 = 0x44;
    pub const ORDER_MODIFIED: u8 = 0x55;
    pub const ORDER_BOOK_CLEAR: u8 = 0x79;
    pub const ORDER_EXECUTED: u8 = 0x45;
    pub const ORDER_EXECUTED_WITH_PRICE_SIZE: u8 = 0x43;
    pub const TRADE: u8 = 0x50;
    pub const AUCTION_TRADE: u8 = 0x51;
    pub const AUCTION_INFO: u8 = 0x49;
    pub const STATISTICS: u8 = 0x77;
    pub const TOP_OF_BOOK: u8 = 0x71;
}

/// Nanoseconds since the most recent `Time` message, accurate to the
/// microsecond. Meaningless without that `Time`, which is why the decoder
/// carries it as state rather than handing back a bare offset.
pub type NanosOffset = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message<'a> {
    /// Seconds since midnight in the **server's local time**, not UTC. Every
    /// other message timestamps itself as an offset from the most recent one.
    Time { seconds: u32 },
    SystemEvent {
        nanos: NanosOffset,
        event: SystemEventCode,
    },
    AddOrder {
        nanos: NanosOffset,
        order_id: u64,
        side: Side,
        quantity: u32,
        instrument_id: u32,
        price: Price,
        flags: Flags,
    },
    AddAttributedOrder {
        nanos: NanosOffset,
        order_id: u64,
        side: Side,
        quantity: u32,
        instrument_id: u32,
        price: Price,
        /// Identity of the submitting firm.
        attribution: &'a str,
        flags: Flags,
    },
    OrderDeleted {
        nanos: NanosOffset,
        order_id: u64,
        flags: Flags,
        instrument_id: u32,
    },
    OrderModified {
        nanos: NanosOffset,
        order_id: u64,
        new_quantity: u32,
        new_price: Price,
        flags: Flags,
    },
    /// Discard the entire book for this instrument. Sent during exchange-side
    /// recovery, followed by a fresh series of Add Order and Statistics.
    OrderBookClear {
        nanos: NanosOffset,
        instrument_id: u32,
        flags: Flags,
    },
    OrderExecuted {
        nanos: NanosOffset,
        order_id: u64,
        executed_quantity: u32,
        trade_match_id: u64,
    },
    OrderExecutedWithPriceSize {
        nanos: NanosOffset,
        order_id: u64,
        executed_quantity: u32,
        display_quantity: u32,
        trade_match_id: u64,
        /// Whether the trade updates time-and-sales and statistics displays.
        printable: bool,
        price: Price,
    },
    Trade {
        nanos: NanosOffset,
        executed_quantity: u32,
        instrument_id: u32,
        price: Price,
        trade_match_id: u64,
    },
    /// A recognised type this codec does not decode. Surfaced rather than
    /// dropped so an unhandled message is countable.
    Unhandled { msg_type: u8, len: usize },
}

impl Message<'_> {
    /// Nanosecond offset from the last `Time`, where the message carries one.
    pub fn nanos(&self) -> Option<NanosOffset> {
        match *self {
            Message::Time { .. } | Message::Unhandled { .. } => None,
            Message::SystemEvent { nanos, .. }
            | Message::AddOrder { nanos, .. }
            | Message::AddAttributedOrder { nanos, .. }
            | Message::OrderDeleted { nanos, .. }
            | Message::OrderModified { nanos, .. }
            | Message::OrderBookClear { nanos, .. }
            | Message::OrderExecuted { nanos, .. }
            | Message::OrderExecutedWithPriceSize { nanos, .. }
            | Message::Trade { nanos, .. } => Some(nanos),
        }
    }

    /// Instrument this message affects, where it names one directly.
    ///
    /// Order-keyed messages — delete, modify, execute — deliberately return
    /// `None` for the ones that carry no instrument id: resolving those needs
    /// the order pool, and guessing here would be worse than saying nothing.
    pub fn instrument_id(&self) -> Option<u32> {
        match *self {
            Message::AddOrder { instrument_id, .. }
            | Message::AddAttributedOrder { instrument_id, .. }
            | Message::OrderDeleted { instrument_id, .. }
            | Message::OrderBookClear { instrument_id, .. }
            | Message::Trade { instrument_id, .. } => Some(instrument_id),
            _ => None,
        }
    }

    pub fn order_id(&self) -> Option<u64> {
        match *self {
            Message::AddOrder { order_id, .. }
            | Message::AddAttributedOrder { order_id, .. }
            | Message::OrderDeleted { order_id, .. }
            | Message::OrderModified { order_id, .. }
            | Message::OrderExecuted { order_id, .. }
            | Message::OrderExecutedWithPriceSize { order_id, .. } => Some(order_id),
            _ => None,
        }
    }
}

/// Minimum bytes each decoded type needs, so a truncated message is reported as
/// a length error rather than as a confusing out-of-bounds read.
fn min_len(ty: u8) -> Option<usize> {
    use msg_type as t;
    Some(match ty {
        t::TIME => 6,
        t::SYSTEM_EVENT => 7,
        t::ADD_ORDER => 34,
        t::ADD_ATTRIBUTED_ORDER => 45,
        t::ORDER_DELETED => 19,
        t::ORDER_MODIFIED => 27,
        t::ORDER_BOOK_CLEAR => 13,
        t::ORDER_EXECUTED => 26,
        t::ORDER_EXECUTED_WITH_PRICE_SIZE => 39,
        t::TRADE => 32,
        _ => return None,
    })
}

/// Decode one message from the front of `buf`.
///
/// `currency` is supplied by the caller: MITCH prices carry no currency of
/// their own, and it is a property of the venue rather than of the message.
pub fn decode<'a>(buf: &'a [u8], currency: Currency) -> Result<(Message<'a>, usize), DecodeError> {
    let len = u8_at(buf, 0)? as usize;
    if len == 0 {
        return Err(DecodeError::ZeroLength);
    }
    let ty = u8_at(buf, 1)?;

    // The declared length bounds this message; a field read may not run past it
    // even when the packet holds more bytes belonging to the next message.
    let body = buf.get(..len).ok_or(DecodeError::Truncated {
        at: 0,
        need: len,
        have: buf.len(),
    })?;

    if let Some(min) = min_len(ty)
        && len < min
    {
        return Err(DecodeError::BadLength {
            ty,
            declared: len,
            expected: min,
        });
    }

    use msg_type as t;
    let msg = match ty {
        t::TIME => Message::Time {
            seconds: u32_at(body, 2)?,
        },

        t::SYSTEM_EVENT => Message::SystemEvent {
            nanos: u32_at(body, 2)?,
            event: SystemEventCode::from_byte(u8_at(body, 6)?)?,
        },

        t::ADD_ORDER => Message::AddOrder {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            side: Side::from_byte(u8_at(body, 14)?)?,
            quantity: u32_at(body, 15)?,
            instrument_id: u32_at(body, 19)?,
            // Offsets 23 and 24 are reserved.
            price: price_at(body, 25, currency)?,
            flags: Flags(u8_at(body, 33)?),
        },

        t::ADD_ATTRIBUTED_ORDER => Message::AddAttributedOrder {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            side: Side::from_byte(u8_at(body, 14)?)?,
            quantity: u32_at(body, 15)?,
            instrument_id: u32_at(body, 19)?,
            price: price_at(body, 25, currency)?,
            attribution: alpha_at(body, 33, 11)?,
            flags: Flags(u8_at(body, 44)?),
        },

        t::ORDER_DELETED => Message::OrderDeleted {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            flags: Flags(u8_at(body, 14)?),
            instrument_id: u32_at(body, 15)?,
        },

        t::ORDER_MODIFIED => Message::OrderModified {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            new_quantity: u32_at(body, 14)?,
            new_price: price_at(body, 18, currency)?,
            flags: Flags(u8_at(body, 26)?),
        },

        t::ORDER_BOOK_CLEAR => Message::OrderBookClear {
            nanos: u32_at(body, 2)?,
            instrument_id: u32_at(body, 6)?,
            // Offsets 10 and 11 are reserved.
            flags: Flags(u8_at(body, 12)?),
        },

        t::ORDER_EXECUTED => Message::OrderExecuted {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            executed_quantity: u32_at(body, 14)?,
            trade_match_id: u64_at(body, 18)?,
        },

        t::ORDER_EXECUTED_WITH_PRICE_SIZE => Message::OrderExecutedWithPriceSize {
            nanos: u32_at(body, 2)?,
            order_id: u64_at(body, 6)?,
            executed_quantity: u32_at(body, 14)?,
            display_quantity: u32_at(body, 18)?,
            trade_match_id: u64_at(body, 22)?,
            printable: match u8_at(body, 30)? {
                b'Y' => true,
                b'N' => false,
                v => {
                    return Err(DecodeError::BadEnum {
                        field: "Printable",
                        value: v,
                        expected: "Y or N",
                    });
                }
            },
            price: price_at(body, 31, currency)?,
        },

        t::TRADE => Message::Trade {
            nanos: u32_at(body, 2)?,
            executed_quantity: u32_at(body, 6)?,
            instrument_id: u32_at(body, 10)?,
            // Offsets 14 and 15 are reserved.
            price: price_at(body, 16, currency)?,
            trade_match_id: u64_at(body, 24)?,
        },

        t::SYMBOL_DIRECTORY
        | t::SYMBOL_STATUS
        | t::AUCTION_TRADE
        | t::AUCTION_INFO
        | t::STATISTICS
        | t::TOP_OF_BOOK => Message::Unhandled { msg_type: ty, len },

        other => return Err(DecodeError::UnknownMessageType(other)),
    };

    Ok((msg, len))
}
