//! Book building from a MITCH message stream.
//!
//! Every scenario is driven through the real decoder — `PacketBuilder` frames
//! synthetic packets, `Packet` decodes them, `BookSet` applies them. Nothing
//! constructs a `Message` by hand, so a change to the wire format that breaks
//! the codec breaks these too rather than passing on a fiction.

use amd_core::Price;
use amd_core::{Currency, DelayClass, ExchangeCode, InstrumentId};
use amd_feed::book::{BookSet, QuoteContext};
use amd_feed::mitch::PRICE_SCALE;
use amd_feed::mitch::{Flags, Packet, PacketBuilder, Side};
use time::macros::datetime;

const INST: u32 = 42;
/// Priority retained is bit 0 of the Order Modified flags.
const KEEP_PRIORITY: Flags = Flags(0b0000_0001);
const LOSE_PRIORITY: Flags = Flags(0b0000_0000);

fn zar(s: &str) -> Price {
    Price::parse(s, Currency::ZAR, PRICE_SCALE).unwrap()
}

fn minor(s: &str) -> i128 {
    zar(s).minor
}

/// Frame a builder into a packet, decode it, and apply every message.
fn drive(books: &mut BookSet, b: &PacketBuilder) {
    let raw = b.finish(1);
    for m in Packet::parse(&raw, Currency::ZAR).unwrap() {
        books.apply(&m.unwrap().message);
    }
}

// ---------------------------------------------------------------- basics

#[test]
fn adds_build_both_sides_and_top_of_book() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("310.50"), Flags(0))
        .add_order(3, 102, Side::Sell, 400, INST, zar("311.00"), Flags(0))
        .add_order(4, 103, Side::Sell, 200, INST, zar("311.50"), Flags(0));
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    // Best bid is the highest price, best ask the lowest.
    assert_eq!(book.best_bid().unwrap().0, minor("310.50"));
    assert_eq!(book.best_ask().unwrap().0, minor("311.00"));
    assert_eq!(book.depth(), (2, 2));
    assert!(!book.is_crossed());
    assert_eq!(books.pool().len(), 4);
    assert_eq!(books.anomalies().total(), 0);
}

#[test]
fn levels_aggregate_size_and_keep_time_priority() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("310.00"), Flags(0))
        .add_order(3, 102, Side::Buy, 200, INST, zar("310.00"), Flags(0));
    drive(&mut books, &b);

    let (_, level) = books.book(INST).unwrap().best_bid().unwrap();
    assert_eq!(level.quantity, 1000);
    assert_eq!(level.order_count(), 3);
    // Arrival order is the queue order — this is what a quote API cannot give.
    assert_eq!(level.queue, vec![100, 101, 102]);
}

#[test]
fn deleting_the_last_order_removes_the_level_entirely() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("309.00"), Flags(0));
    drive(&mut books, &b);
    assert_eq!(books.book(INST).unwrap().depth(), (2, 0));

    let mut b = PacketBuilder::new(1);
    b.order_deleted(3, 100, Flags(0), INST);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert_eq!(book.depth(), (1, 0), "an emptied level must not linger");
    assert_eq!(book.best_bid().unwrap().0, minor("309.00"));
    assert_eq!(books.pool().len(), 1);
}

// ---------------------------------------------------------------- priority

#[test]
fn a_resize_at_the_same_price_keeps_queue_position() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("310.00"), Flags(0))
        .order_modified(3, 100, 250, zar("310.00"), KEEP_PRIORITY);
    drive(&mut books, &b);

    let (_, level) = books.book(INST).unwrap().best_bid().unwrap();
    assert_eq!(level.queue, vec![100, 101], "order 100 stays at the front");
    assert_eq!(level.quantity, 550, "500 → 250, plus 300");
}

#[test]
fn losing_priority_sends_the_order_to_the_back() {
    // The exchange can revoke priority even at an unchanged price. Ignoring
    // the flag silently mis-ranks the level.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("310.00"), Flags(0))
        .order_modified(3, 100, 600, zar("310.00"), LOSE_PRIORITY);
    drive(&mut books, &b);

    let (_, level) = books.book(INST).unwrap().best_bid().unwrap();
    assert_eq!(level.queue, vec![101, 100], "order 100 goes to the back");
    assert_eq!(level.quantity, 900);
}

#[test]
fn a_price_move_always_joins_the_back_of_the_new_level() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Buy, 300, INST, zar("309.00"), Flags(0))
        // Even claiming retained priority, a price change cannot keep a place
        // in a queue the order was never in.
        .order_modified(3, 100, 500, zar("309.00"), KEEP_PRIORITY);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert_eq!(book.depth(), (1, 0), "the vacated level is gone");
    let (price, level) = book.best_bid().unwrap();
    assert_eq!(price, minor("309.00"));
    assert_eq!(level.queue, vec![101, 100]);
    assert_eq!(level.quantity, 800);
}

// ---------------------------------------------------------------- executions

#[test]
fn a_partial_fill_reduces_size_without_costing_the_queue_place() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Sell, 500, INST, zar("311.00"), Flags(0))
        .add_order(2, 101, Side::Sell, 300, INST, zar("311.00"), Flags(0))
        .order_executed(3, 100, 200, 9001);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    let (_, level) = book.best_ask().unwrap();
    assert_eq!(level.quantity, 600, "500-200 remaining, plus 300");
    assert_eq!(level.queue, vec![100, 101], "the remainder keeps its place");
    assert_eq!(book.last_trade(), Some((minor("311.00"), 200)));
    assert_eq!(book.volume(), 200);
}

#[test]
fn a_full_fill_removes_the_order_from_book_and_pool() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Sell, 500, INST, zar("311.00"), Flags(0))
        .order_executed(2, 100, 500, 9001);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert_eq!(book.depth(), (0, 0));
    assert!(books.pool().is_empty());
    assert_eq!(book.volume(), 500);
    assert_eq!(books.anomalies().total(), 0);
}

#[test]
fn a_non_display_trade_moves_the_tape_but_not_the_book() {
    // A Trade message reports a fill of an order that was never visible, so
    // there is nothing in the book to remove.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .trade(2, 750, INST, zar("310.25"), 9002);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert_eq!(book.depth(), (1, 0), "the resting order is untouched");
    assert_eq!(book.last_trade(), Some((minor("310.25"), 750)));
    assert_eq!(book.volume(), 750);
    assert_eq!(books.pool().len(), 1);
}

// ---------------------------------------------------------------- recovery

#[test]
fn order_book_clear_wipes_resting_orders_but_keeps_the_tape() {
    // Sent during exchange-side recovery, followed by re-published Add Orders.
    // The exchange is restating the book, not retracting trades that printed.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 101, Side::Sell, 400, INST, zar("311.00"), Flags(0))
        .trade(3, 250, INST, zar("310.50"), 9003);
    drive(&mut books, &b);
    assert_eq!(books.book(INST).unwrap().depth(), (1, 1));

    let mut b = PacketBuilder::new(1);
    b.order_book_clear(4, INST, Flags(0));
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert!(book.is_empty(), "resting orders are gone");
    assert_eq!(
        book.last_trade(),
        Some((minor("310.50"), 250)),
        "the tape survives"
    );
    assert_eq!(book.volume(), 250);
    assert!(books.pool().is_empty());
}

#[test]
fn a_clear_touches_only_the_named_instrument() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 200, Side::Buy, 500, 99, zar("50.00"), Flags(0));
    drive(&mut books, &b);

    let mut b = PacketBuilder::new(1);
    b.order_book_clear(3, INST, Flags(0));
    drive(&mut books, &b);

    assert!(books.book(INST).unwrap().is_empty());
    assert_eq!(
        books.book(99).unwrap().depth(),
        (1, 0),
        "the other book is untouched"
    );
    assert_eq!(books.pool().len(), 1);
}

// ---------------------------------------------------------------- anomalies

#[test]
fn messages_for_unknown_orders_are_counted_not_swallowed() {
    // The signature of a gap that recovery missed. A handler that halts on the
    // first inconsistency is useless, but one that hides them is worse.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.order_deleted(1, 555, Flags(0), INST)
        .order_executed(2, 556, 100, 9004)
        .order_modified(3, 557, 100, zar("1.00"), KEEP_PRIORITY);
    drive(&mut books, &b);

    assert_eq!(books.anomalies().unknown_order, 3);
    assert_eq!(books.anomalies().total(), 3);
}

#[test]
fn re_adding_a_live_order_id_does_not_double_count_size() {
    // Means we missed the delete. Replaying the add over the stale entry would
    // inflate the level, which is worse than the gap that caused it.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 100, Side::Buy, 300, INST, zar("310.00"), Flags(0));
    drive(&mut books, &b);

    let (_, level) = books.book(INST).unwrap().best_bid().unwrap();
    assert_eq!(
        level.quantity, 300,
        "the stale entry is replaced, not added to"
    );
    assert_eq!(level.order_count(), 1);
    assert_eq!(books.anomalies().duplicate_order, 1);
}

#[test]
fn an_oversized_execution_is_flagged_and_clamped() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Sell, 100, INST, zar("311.00"), Flags(0))
        .order_executed(2, 100, 250, 9005);
    drive(&mut books, &b);

    assert_eq!(books.anomalies().oversized_execution, 1);
    // Clamped rather than wrapped: the order is fully gone, not left with
    // 4.29 billion shares.
    assert!(books.book(INST).unwrap().is_empty());
    assert!(books.pool().is_empty());
}

// ---------------------------------------------------------------- crossing

#[test]
fn a_crossed_book_is_detectable() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("311.50"), Flags(0))
        .add_order(2, 101, Side::Sell, 400, INST, zar("311.00"), Flags(0));
    drive(&mut books, &b);

    assert!(books.book(INST).unwrap().is_crossed(), "bid above ask");
}

// ---------------------------------------------------------------- quote

#[test]
fn the_book_projects_into_the_same_quote_shape_the_adapters_produce() {
    // The whole point: a licensed direct feed must be substitutable for a free
    // polled source without anything downstream noticing.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.50"), Flags(0))
        .add_order(2, 101, Side::Sell, 400, INST, zar("311.00"), Flags(0))
        .trade(3, 250, INST, zar("310.75"), 9006);
    drive(&mut books, &b);

    let as_of = datetime!(2026-08-18 09:15:00 UTC);
    let ctx = QuoteContext::new(
        InstrumentId::new(ExchangeCode::Jse, "NPN"),
        Currency::ZAR,
        "jse-mitch",
        as_of,
        DelayClass::Realtime,
    )
    .received_at(datetime!(2026-08-18 09:15:00.000180 UTC))
    .sequence(1_234_567);
    let quote = books.book(INST).unwrap().to_quote(&ctx);

    assert_eq!(quote.bid, Some(zar("310.50")));
    assert_eq!(quote.ask, Some(zar("311.00")));
    assert_eq!(quote.last, Some(zar("310.75")));
    assert_eq!(quote.volume, Some(250));
    assert_eq!(quote.trades, Some(1));
    assert_eq!(quote.is_crossed(), Some(false));

    // Provenance a REST source cannot produce.
    assert_eq!(quote.provenance.source, "jse-mitch");
    assert_eq!(quote.provenance.delay, DelayClass::Realtime);
    assert!(
        !quote.provenance.as_of_imputed,
        "a feed stamps its own data"
    );
    assert_eq!(quote.provenance.sequence, Some(1_234_567));
    assert!(!quote.provenance.recovered);
    // Sub-millisecond lag rounds to zero whole seconds, which is correct.
    assert_eq!(quote.provenance.lag_seconds(), 0);
}

#[test]
fn quote_prices_stay_at_mitch_scale_end_to_end() {
    // No rescaling anywhere between the wire and the quote, so nothing rounds.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 1, INST, zar("0.00000001"), Flags(0));
    drive(&mut books, &b);

    let ctx = QuoteContext::new(
        InstrumentId::new(ExchangeCode::Jse, "TINY"),
        Currency::ZAR,
        "jse-mitch",
        datetime!(2026-08-18 09:00:00 UTC),
        DelayClass::Realtime,
    );
    let quote = books.book(INST).unwrap().to_quote(&ctx);
    let bid = quote.bid.unwrap();
    assert_eq!(bid.scale, PRICE_SCALE);
    assert_eq!(bid.minor, 1, "one minor unit survives the whole path");
}

// ---------------------------------------------------------------- session

#[test]
fn a_realistic_session_leaves_a_consistent_book() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.time(34_200)
        .add_order(1, 1, Side::Buy, 1000, INST, zar("310.00"), Flags(0))
        .add_order(2, 2, Side::Buy, 800, INST, zar("310.25"), Flags(0))
        .add_order(3, 3, Side::Sell, 900, INST, zar("311.00"), Flags(0))
        .add_order(4, 4, Side::Sell, 600, INST, zar("310.75"), Flags(0))
        // Someone improves the bid.
        .order_modified(5, 1, 1000, zar("310.50"), LOSE_PRIORITY)
        // Partial fill on the best ask.
        .order_executed(6, 4, 200, 1)
        // The other side pulls.
        .order_deleted(7, 3, Flags(0), INST)
        // A dark fill prints.
        .trade(8, 400, INST, zar("310.60"), 2);
    drive(&mut books, &b);

    let book = books.book(INST).unwrap();
    assert_eq!(
        book.best_bid().unwrap().0,
        minor("310.50"),
        "the improved bid leads"
    );
    assert_eq!(book.best_ask().unwrap().0, minor("310.75"));
    // Order 1 *moved* from 310.00 to 310.50 rather than adding a level, so the
    // 310.00 level is vacated and two bids remain.
    assert_eq!(book.depth(), (2, 1), "310.50 and 310.25 bid; 310.75 ask");
    assert_eq!(
        book.best_ask().unwrap().1.quantity,
        400,
        "600 less the 200 filled"
    );
    assert_eq!(book.volume(), 600, "200 displayed + 400 dark");
    assert_eq!(book.last_trade(), Some((minor("310.60"), 400)));
    assert!(!book.is_crossed());
    assert_eq!(
        books.anomalies().total(),
        0,
        "a clean session leaves no anomalies"
    );
    assert_eq!(books.pool().len(), 3);
}

#[test]
fn reset_clears_everything_for_an_exchange_restart() {
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0))
        .add_order(2, 200, Side::Sell, 500, 99, zar("50.00"), Flags(0));
    drive(&mut books, &b);
    assert_eq!(books.instruments().count(), 2);

    // On failover the sequence resets to 1 and the previous session's books
    // are meaningless.
    books.reset();
    assert_eq!(books.instruments().count(), 0);
    assert!(books.pool().is_empty());
}

#[test]
fn a_recovered_book_is_flagged_so_analytics_can_exclude_it() {
    // Snapshot recovery does not preserve original timestamps, so a book
    // rebuilt that way must be distinguishable from one built live.
    let mut books = BookSet::new();
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 100, Side::Buy, 500, INST, zar("310.00"), Flags(0));
    drive(&mut books, &b);

    let ctx = QuoteContext::new(
        InstrumentId::new(ExchangeCode::Jse, "NPN"),
        Currency::ZAR,
        "jse-mitch",
        datetime!(2026-08-18 09:00:00 UTC),
        DelayClass::Realtime,
    )
    .recovered();
    let quote = books.book(INST).unwrap().to_quote(&ctx);

    assert!(quote.provenance.recovered);
    assert!(
        !quote.provenance.as_of_imputed,
        "recovered is not the same as imputed"
    );
}
