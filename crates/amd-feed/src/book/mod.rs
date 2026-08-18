//! Book building from a MITCH message stream.
//!
//! Turns the decoded feed into per-instrument order books and, from those, the
//! same [`amd_core::Quote`] the polled adapters produce — which is what lets a
//! licensed direct feed replace a free source without anything downstream
//! noticing.
//!
//! ```text
//! Message ──▶ BookSet::apply ──▶ Book (bids/asks + last trade) ──▶ Quote
//!                   │
//!                   └── OrderPool resolves the order-keyed messages that
//!                       carry no instrument id of their own
//! ```

pub mod level;
pub mod order;

use std::collections::{BTreeMap, HashMap};

use amd_core::{Currency, DelayClass, InstrumentId, Price, Provenance, Quote};
use time::OffsetDateTime;

use crate::mitch::{Message, PRICE_SCALE, Side};

pub use level::Level;
pub use order::{Order, OrderPool};

/// Something the stream asked for that the book could not do.
///
/// These are not fatal — a feed handler that halts on the first inconsistency
/// is useless — but they must be counted rather than swallowed. A rising
/// `unknown_order` count is the signature of a gap that recovery missed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Anomalies {
    /// A message referenced an order the pool has never seen.
    pub unknown_order: u64,
    /// An order was in the pool but missing from its price level.
    pub desynced_level: u64,
    /// An execution claimed more size than the order displayed.
    pub oversized_execution: u64,
    /// An add arrived for an order id already live.
    pub duplicate_order: u64,
}

impl Anomalies {
    pub fn total(&self) -> u64 {
        self.unknown_order + self.desynced_level + self.oversized_execution + self.duplicate_order
    }
}

/// Everything a book needs from its caller to stamp a [`Quote`].
///
/// A struct rather than a long argument list because none of these are
/// properties of the book: the delay class comes from the licence the feed is
/// received under, the timestamps from the packet and the receive path, and the
/// instrument identity from symbology. The book knows prices and sizes; it does
/// not know under what terms it may be published.
#[derive(Debug, Clone)]
pub struct QuoteContext<'a> {
    pub instrument: InstrumentId,
    pub currency: Currency,
    /// Feed id recorded in the provenance, e.g. `"jse-mitch"`.
    pub source: &'a str,
    /// Exchange timestamp, reconstructed from the last `Time` message plus the
    /// message's own nanosecond offset.
    pub as_of: OffsetDateTime,
    /// When the packet reached this process. PTP-disciplined in colocation.
    pub received_at: OffsetDateTime,
    pub delay: DelayClass,
    /// Sequence of the message that produced this state.
    pub sequence: u64,
    /// Set when the book was rebuilt through a recovery path. Snapshot recovery
    /// does not preserve original timestamps, so anything flagged here must be
    /// excluded from microstructure analytics.
    pub recovered: bool,
}

impl<'a> QuoteContext<'a> {
    pub fn new(
        instrument: InstrumentId,
        currency: Currency,
        source: &'a str,
        as_of: OffsetDateTime,
        delay: DelayClass,
    ) -> Self {
        QuoteContext {
            instrument,
            currency,
            source,
            as_of,
            received_at: as_of,
            delay,
            sequence: 0,
            recovered: false,
        }
    }

    pub fn received_at(mut self, at: OffsetDateTime) -> Self {
        self.received_at = at;
        self
    }

    pub fn sequence(mut self, sequence: u64) -> Self {
        self.sequence = sequence;
        self
    }

    pub fn recovered(mut self) -> Self {
        self.recovered = true;
        self
    }
}

/// One instrument's book.
#[derive(Debug)]
pub struct Book {
    pub instrument_id: u32,
    /// Descending by price: the best bid is the last key.
    bids: BTreeMap<i128, Level>,
    /// Ascending by price: the best ask is the first key.
    asks: BTreeMap<i128, Level>,
    last_trade: Option<(i128, u32)>,
    /// Cumulative traded volume this session.
    volume: u64,
    trades: u32,
}

impl Book {
    pub fn new(instrument_id: u32) -> Self {
        Book {
            instrument_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_trade: None,
            volume: 0,
            trades: 0,
        }
    }

    fn side_mut(&mut self, side: Side) -> &mut BTreeMap<i128, Level> {
        match side {
            Side::Buy => &mut self.bids,
            Side::Sell => &mut self.asks,
        }
    }

    /// Best bid: highest price anyone is willing to pay.
    pub fn best_bid(&self) -> Option<(i128, &Level)> {
        self.bids.iter().next_back().map(|(p, l)| (*p, l))
    }

    /// Best ask: lowest price anyone is willing to sell at.
    pub fn best_ask(&self) -> Option<(i128, &Level)> {
        self.asks.iter().next().map(|(p, l)| (*p, l))
    }

    /// Bids from best downward.
    pub fn bid_levels(&self) -> impl Iterator<Item = (i128, &Level)> {
        self.bids.iter().rev().map(|(p, l)| (*p, l))
    }

    /// Asks from best upward.
    pub fn ask_levels(&self) -> impl Iterator<Item = (i128, &Level)> {
        self.asks.iter().map(|(p, l)| (*p, l))
    }

    pub fn depth(&self) -> (usize, usize) {
        (self.bids.len(), self.asks.len())
    }

    pub fn is_empty(&self) -> bool {
        self.bids.is_empty() && self.asks.is_empty()
    }

    /// Whether the best bid sits at or above the best ask.
    ///
    /// A crossed book is either a decoder bug or a genuine venue condition
    /// during an auction. Both are worth catching before publishing, not after
    /// a user reports it.
    pub fn is_crossed(&self) -> bool {
        match (self.best_bid(), self.best_ask()) {
            (Some((b, _)), Some((a, _))) => b >= a,
            _ => false,
        }
    }

    pub fn last_trade(&self) -> Option<(i128, u32)> {
        self.last_trade
    }

    pub fn volume(&self) -> u64 {
        self.volume
    }

    /// Project the book into the workspace-wide [`Quote`] shape.
    pub fn to_quote(&self, ctx: &QuoteContext<'_>) -> Quote {
        let currency = ctx.currency;
        let px = |minor: i128| Price {
            minor,
            scale: PRICE_SCALE,
            currency,
        };
        let mut provenance = Provenance::stamped(ctx.source, ctx.as_of, ctx.received_at, ctx.delay)
            .with_sequence(ctx.sequence);
        if ctx.recovered {
            provenance = provenance.recovered();
        }

        let mut q = Quote::new(ctx.instrument.clone(), provenance);
        q.bid = self.best_bid().map(|(p, _)| px(p));
        q.ask = self.best_ask().map(|(p, _)| px(p));
        q.last = self.last_trade.map(|(p, _)| px(p));
        q.volume = (self.volume > 0).then_some(self.volume);
        q.trades = (self.trades > 0).then_some(self.trades);
        q
    }
}

/// Every book for one venue, plus the order pool they share.
#[derive(Debug)]
pub struct BookSet {
    books: HashMap<u32, Book>,
    pool: OrderPool,
    anomalies: Anomalies,
}

impl Default for BookSet {
    fn default() -> Self {
        Self::new()
    }
}

impl BookSet {
    pub fn new() -> Self {
        BookSet {
            books: HashMap::new(),
            pool: OrderPool::new(),
            anomalies: Anomalies::default(),
        }
    }

    pub fn book(&self, instrument_id: u32) -> Option<&Book> {
        self.books.get(&instrument_id)
    }

    pub fn instruments(&self) -> impl Iterator<Item = u32> + '_ {
        self.books.keys().copied()
    }

    pub fn anomalies(&self) -> Anomalies {
        self.anomalies
    }

    pub fn pool(&self) -> &OrderPool {
        &self.pool
    }

    fn book_mut(&mut self, instrument_id: u32) -> &mut Book {
        self.books
            .entry(instrument_id)
            .or_insert_with(|| Book::new(instrument_id))
    }

    /// Apply one decoded message.
    ///
    /// Returns the instrument the message touched, so a caller can republish
    /// just that book rather than diffing the venue. `None` means the message
    /// changed no book — a `Time`, a `System Event`, or one that could not be
    /// resolved.
    pub fn apply(&mut self, message: &Message<'_>) -> Option<u32> {
        match *message {
            Message::AddOrder {
                order_id,
                side,
                quantity,
                instrument_id,
                price,
                ..
            } => self.add(order_id, instrument_id, side, price.minor, quantity),
            Message::AddAttributedOrder {
                order_id,
                side,
                quantity,
                instrument_id,
                price,
                ..
            } => self.add(order_id, instrument_id, side, price.minor, quantity),

            Message::OrderModified {
                order_id,
                new_quantity,
                new_price,
                flags,
                ..
            } => {
                // Bit 0 is the priority flag: set means priority retained.
                self.modify(order_id, new_quantity, new_price.minor, flags.bit(0))
            }

            Message::OrderDeleted { order_id, .. } => self.delete(order_id),

            Message::OrderExecuted {
                order_id,
                executed_quantity,
                ..
            } => self.execute(order_id, executed_quantity, None),

            Message::OrderExecutedWithPriceSize {
                order_id,
                executed_quantity,
                display_quantity,
                price,
                printable,
                ..
            } => {
                let touched = self.execute_to_size(order_id, executed_quantity, display_quantity);
                // A non-printable execution does not update time-and-sales or
                // statistics displays, so it must not move `last`.
                if printable && let Some(id) = touched {
                    let book = self.book_mut(id);
                    book.last_trade = Some((price.minor, executed_quantity));
                    book.volume += executed_quantity as u64;
                    book.trades += 1;
                }
                touched
            }

            Message::Trade {
                executed_quantity,
                instrument_id,
                price,
                ..
            } => {
                // A non-display order filled. It never sat in the visible book,
                // so there is nothing to remove — only the tape to update.
                let book = self.book_mut(instrument_id);
                book.last_trade = Some((price.minor, executed_quantity));
                book.volume += executed_quantity as u64;
                book.trades += 1;
                Some(instrument_id)
            }

            Message::OrderBookClear { instrument_id, .. } => {
                self.pool.clear_instrument(instrument_id);
                // Keep the tape: the exchange is re-publishing resting orders,
                // not retracting the trades that already printed.
                if let Some(book) = self.books.get_mut(&instrument_id) {
                    book.bids.clear();
                    book.asks.clear();
                }
                Some(instrument_id)
            }

            Message::Time { .. } | Message::SystemEvent { .. } | Message::Unhandled { .. } => None,
        }
    }

    fn add(
        &mut self,
        order_id: u64,
        instrument_id: u32,
        side: Side,
        price: i128,
        quantity: u32,
    ) -> Option<u32> {
        if self.pool.get(order_id).is_some() {
            // Re-adding a live id means we missed its delete. Replaying the add
            // over the stale one would double-count size, so drop the old first.
            self.anomalies.duplicate_order += 1;
            self.delete(order_id);
        }
        self.pool.insert(
            order_id,
            Order {
                instrument_id,
                side,
                price,
                quantity,
            },
        );
        let book = self.book_mut(instrument_id);
        book.side_mut(side)
            .entry(price)
            .or_default()
            .push_back(order_id, quantity);
        Some(instrument_id)
    }

    fn delete(&mut self, order_id: u64) -> Option<u32> {
        let Some(order) = self.pool.remove(order_id) else {
            self.anomalies.unknown_order += 1;
            return None;
        };
        self.remove_from_level(order_id, order);
        Some(order.instrument_id)
    }

    fn remove_from_level(&mut self, order_id: u64, order: Order) {
        let desynced = {
            let book = self.book_mut(order.instrument_id);
            let levels = book.side_mut(order.side);
            match levels.get_mut(&order.price) {
                Some(level) => {
                    let found = level.remove(order_id, order.quantity);
                    if level.is_empty() {
                        levels.remove(&order.price);
                    }
                    !found
                }
                None => true,
            }
        };
        if desynced {
            self.anomalies.desynced_level += 1;
        }
    }

    fn modify(
        &mut self,
        order_id: u64,
        new_quantity: u32,
        new_price: i128,
        priority_retained: bool,
    ) -> Option<u32> {
        let Some(old) = self.pool.get(order_id) else {
            self.anomalies.unknown_order += 1;
            return None;
        };

        // Priority survives only an in-place size change at the same price.
        // A price move always goes to the back of the new level, and the
        // exchange can also revoke priority outright — ignoring either silently
        // mis-ranks the queue.
        if priority_retained && new_price == old.price {
            let book = self.book_mut(old.instrument_id);
            if let Some(level) = book.side_mut(old.side).get_mut(&old.price) {
                level.resize(old.quantity, new_quantity);
            } else {
                self.anomalies.desynced_level += 1;
            }
        } else {
            self.remove_from_level(order_id, old);
            let book = self.book_mut(old.instrument_id);
            book.side_mut(old.side)
                .entry(new_price)
                .or_default()
                .push_back(order_id, new_quantity);
        }

        if let Some(o) = self.pool.get_mut(order_id) {
            o.price = new_price;
            o.quantity = new_quantity;
        }
        Some(old.instrument_id)
    }

    /// Execute against an order, reducing its displayed size by `executed`.
    fn execute(&mut self, order_id: u64, executed: u32, at_price: Option<i128>) -> Option<u32> {
        let Some(order) = self.pool.get(order_id) else {
            self.anomalies.unknown_order += 1;
            return None;
        };
        if executed > order.quantity {
            self.anomalies.oversized_execution += 1;
        }
        let remaining = order.quantity.saturating_sub(executed);
        let id = self.resize_or_remove(order_id, order, remaining);

        let price = at_price.unwrap_or(order.price);
        let book = self.book_mut(order.instrument_id);
        book.last_trade = Some((price, executed));
        book.volume += executed as u64;
        book.trades += 1;
        id
    }

    /// Execute where the venue states the resulting displayed size directly.
    fn execute_to_size(&mut self, order_id: u64, executed: u32, display: u32) -> Option<u32> {
        let Some(order) = self.pool.get(order_id) else {
            self.anomalies.unknown_order += 1;
            return None;
        };
        if executed > order.quantity {
            self.anomalies.oversized_execution += 1;
        }
        self.resize_or_remove(order_id, order, display)
    }

    fn resize_or_remove(&mut self, order_id: u64, order: Order, remaining: u32) -> Option<u32> {
        if remaining == 0 {
            self.pool.remove(order_id);
            self.remove_from_level(order_id, order);
        } else {
            // A partial fill reduces size but does not cost the remainder its
            // place in the queue.
            let book = self.book_mut(order.instrument_id);
            if let Some(level) = book.side_mut(order.side).get_mut(&order.price) {
                level.resize(order.quantity, remaining);
            } else {
                self.anomalies.desynced_level += 1;
            }
            if let Some(o) = self.pool.get_mut(order_id) {
                o.quantity = remaining;
            }
        }
        Some(order.instrument_id)
    }

    /// Drop everything. Used on an exchange restart, where the sequence resets
    /// and the previous session's books are meaningless.
    pub fn reset(&mut self) {
        self.books.clear();
        self.pool.clear();
    }
}
