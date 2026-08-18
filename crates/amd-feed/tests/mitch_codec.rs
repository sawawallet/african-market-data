//! Codec conformance against the MIT303 wire format.
//!
//! Everything here runs offline against synthetic packets. That is the point:
//! the codec is verifiable before any market data agreement exists.

use amd_core::{Currency, Price};
use amd_feed::mitch::{
    DecodeError, Flags, Message, PRICE_SCALE, Packet, PacketBuilder, Side, SystemEventCode,
    UNIT_HEADER_LEN, UnitHeader, message::decode,
};

fn zar(s: &str) -> Price {
    Price::parse(s, Currency::ZAR, PRICE_SCALE).unwrap()
}

// ---------------------------------------------------------------- header

#[test]
fn unit_header_matches_the_documented_byte_layout() {
    // length=20, count=3, group=7, sequence=0x0000_2A00, then 12 payload bytes.
    let mut raw = vec![0x14, 0x00, 0x03, 0x07, 0x00, 0x2A, 0x00, 0x00];
    raw.extend_from_slice(&[0u8; 12]);
    let h = UnitHeader::decode(&raw).unwrap();
    assert_eq!(h.length, 20);
    assert_eq!(h.message_count, 3);
    assert_eq!(h.market_data_group, 7);
    assert_eq!(h.sequence, 0x2A00);
}

#[test]
fn next_expected_is_sequence_plus_count() {
    // The one identity the whole gap detector rests on.
    let raw = PacketBuilder::heartbeat(1, 500);
    let h = UnitHeader::decode(&raw).unwrap();
    assert_eq!(h.message_count, 0);
    assert_eq!(
        h.next_expected(),
        500,
        "a heartbeat must not advance the sequence"
    );
    assert!(h.is_heartbeat());

    let mut b = PacketBuilder::new(1);
    b.time(100).time(101).time(102);
    let raw = b.finish(500);
    let h = UnitHeader::decode(&raw).unwrap();
    assert_eq!(h.next_expected(), 503);
}

#[test]
fn a_header_longer_than_its_packet_is_rejected() {
    let mut raw = PacketBuilder::heartbeat(1, 1);
    raw[0] = 0xFF; // claim 255 bytes in an 8-byte datagram
    assert!(matches!(
        UnitHeader::decode(&raw),
        Err(DecodeError::BadUnitLength {
            declared: 255,
            actual: 8
        })
    ));
}

// ---------------------------------------------------------------- messages

#[test]
fn add_order_round_trips_every_field() {
    let mut b = PacketBuilder::new(1);
    b.add_order(
        123_456,
        0xDEAD_BEEF_CAFE,
        Side::Sell,
        900,
        42,
        zar("310.55"),
        Flags(0b0001_0000),
    );
    let raw = b.finish(1);

    let msgs: Vec<_> = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(msgs.len(), 1);

    match msgs[0].message {
        Message::AddOrder {
            nanos,
            order_id,
            side,
            quantity,
            instrument_id,
            price,
            flags,
        } => {
            assert_eq!(nanos, 123_456);
            assert_eq!(order_id, 0xDEAD_BEEF_CAFE);
            assert_eq!(side, Side::Sell);
            assert_eq!(quantity, 900);
            assert_eq!(instrument_id, 42);
            assert_eq!(price, zar("310.55"));
            assert!(flags.bit(4), "market order flag");
            assert!(!flags.bit(6), "private RFQ flag");
        }
        ref other => panic!("expected AddOrder, got {other:?}"),
    }
}

#[test]
fn prices_land_at_mitch_scale_without_rescaling() {
    // The reason DEFAULT_SCALE is 8: an exchange-native price needs no
    // conversion, so nothing rounds at ingestion.
    let mut b = PacketBuilder::new(1);
    b.trade(1, 100, 7, zar("0.00000001"), 99); // one minor unit
    let raw = b.finish(1);
    let m = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    match m.message {
        Message::Trade { price, .. } => {
            assert_eq!(price.scale, PRICE_SCALE);
            assert_eq!(price.minor, 1);
            assert_eq!(price.currency, Currency::ZAR);
        }
        ref other => panic!("expected Trade, got {other:?}"),
    }
}

#[test]
fn negative_prices_survive_the_signed_decode() {
    // Price is a *signed* i64 on the wire. Spreads and some derivative marks
    // are legitimately negative; decoding them as unsigned would produce a
    // number near 1.8e19 rather than a small negative.
    let mut b = PacketBuilder::new(1);
    b.order_modified(1, 5, 100, zar("-12.25"), Flags(0));
    let raw = b.finish(1);
    let m = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    match m.message {
        Message::OrderModified { new_price, .. } => {
            assert_eq!(new_price, zar("-12.25"));
            assert!(new_price.minor < 0);
        }
        ref other => panic!("expected OrderModified, got {other:?}"),
    }
}

#[test]
fn order_modified_reports_whether_priority_was_retained() {
    // Bit 0 is the priority flag. Losing it moves the order to the back of the
    // queue at its price level; a book builder that ignores this silently
    // mis-ranks the level.
    for (bits, retained) in [(0b0000_0001u8, true), (0b0000_0000, false)] {
        let mut b = PacketBuilder::new(1);
        b.order_modified(1, 5, 100, zar("10.00"), Flags(bits));
        let raw = b.finish(1);
        let m = Packet::parse(&raw, Currency::ZAR)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        match m.message {
            Message::OrderModified { flags, .. } => assert_eq!(flags.bit(0), retained),
            ref other => panic!("expected OrderModified, got {other:?}"),
        }
    }
}

#[test]
fn system_event_decodes_start_and_end_of_day() {
    for (code, want) in [
        (b'O', SystemEventCode::StartOfDay),
        (b'C', SystemEventCode::EndOfDay),
    ] {
        let mut b = PacketBuilder::new(1);
        b.system_event(7, code);
        let raw = b.finish(1);
        let m = Packet::parse(&raw, Currency::ZAR)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        assert_eq!(
            m.message,
            Message::SystemEvent {
                nanos: 7,
                event: want
            }
        );
    }
}

#[test]
fn invalid_enum_values_are_errors_not_guesses() {
    let mut b = PacketBuilder::new(1);
    b.add_order(1, 1, Side::Buy, 1, 1, zar("1.00"), Flags(0));
    let mut raw = b.finish(1);
    // MIT303 offsets are relative to the start of the message, where offset 0
    // is the message's own Length byte — so Side at message offset 14 sits at
    // packet offset 8 + 14, not 8 + 1 + 14.
    let side_at = UNIT_HEADER_LEN + 14;
    raw[side_at] = b'X';
    let err = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .next()
        .unwrap()
        .unwrap_err();
    assert!(matches!(
        err,
        DecodeError::BadEnum {
            field: "Side",
            value: b'X',
            ..
        }
    ));
}

#[test]
fn recognised_but_undecoded_types_surface_rather_than_vanish() {
    // Statistics (0x77) is recognised and skipped. It must be countable, not
    // silently discarded — an invisible unhandled message is how you discover
    // months later that closing prices were never captured.
    let raw = [
        0x0Bu8, 0x00, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, // header: len 11, count 1, seq 1
        0x03, 0x77, 0x00, // message: len 3, type 0x77, one byte body
    ];
    let m = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    assert_eq!(
        m.message,
        Message::Unhandled {
            msg_type: 0x77,
            len: 3
        }
    );
}

#[test]
fn an_unknown_message_type_is_an_error() {
    let raw = [
        0x0Bu8, 0x00, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, //
        0x03, 0xFE, 0x00,
    ];
    let err = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .next()
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, DecodeError::UnknownMessageType(0xFE)));
}

#[test]
fn a_truncated_message_is_a_length_error_not_a_panic() {
    // Claim an Add Order but supply far too few bytes.
    let mut short = vec![0x08u8, 0x41];
    short.extend_from_slice(&[0; 6]);
    let err = decode(&short, Currency::ZAR).unwrap_err();
    assert!(matches!(
        err,
        DecodeError::BadLength {
            ty: 0x41,
            declared: 8,
            expected: 34
        }
    ));
}

#[test]
fn a_zero_length_message_cannot_stall_the_parser() {
    // A length of zero would leave the offset unchanged and loop forever.
    let err = decode(&[0x00, 0x41], Currency::ZAR).unwrap_err();
    assert_eq!(err, DecodeError::ZeroLength);
}

// ---------------------------------------------------------------- sequencing

#[test]
fn sequence_numbers_are_implied_across_a_packet() {
    // The wire carries the first sequence only; the rest are +1 each. Getting
    // this wrong makes every downstream recovery decision wrong.
    let mut b = PacketBuilder::new(1);
    b.time(3600)
        .add_order(1, 10, Side::Buy, 100, 1, zar("5.00"), Flags(0))
        .add_order(2, 11, Side::Sell, 200, 1, zar("5.10"), Flags(0))
        .order_deleted(3, 10, Flags(0), 1);
    let raw = b.finish(1000);

    let packet = Packet::parse(&raw, Currency::ZAR).unwrap();
    let header = packet.header();
    let seqs: Vec<u32> = packet.map(|m| m.unwrap().sequence).collect();

    assert_eq!(seqs, vec![1000, 1001, 1002, 1003]);
    assert_eq!(header.next_expected(), 1004);
}

#[test]
fn message_count_mismatch_is_detectable() {
    let mut b = PacketBuilder::new(1);
    b.time(1).time(2);
    let mut raw = b.finish(1);
    raw[2] = 5; // header claims five messages, payload holds two

    let mut packet = Packet::parse(&raw, Currency::ZAR).unwrap();
    let found = packet.by_ref().filter(Result::is_ok).count();
    assert_eq!(found, 2);
    assert!(matches!(
        packet.verify_count(),
        Err(DecodeError::MessageCountMismatch {
            declared: 5,
            found: 2
        })
    ));
}

#[test]
fn unsequenced_traffic_carries_sequence_zero() {
    // Administrative messages and everything on the Recovery channel are
    // unsequenced. Assigning them numbers would corrupt the gap detector.
    let mut b = PacketBuilder::new(1);
    b.time(1).time(2);
    let raw = b.finish(0);
    let seqs: Vec<u32> = Packet::parse(&raw, Currency::ZAR)
        .unwrap()
        .map(|m| m.unwrap().sequence)
        .collect();
    assert_eq!(seqs, vec![0, 0]);
}

#[test]
fn parsing_stops_at_the_first_bad_message() {
    // Continuing past a decode failure would emit sequence numbers from a
    // parser that has lost its place in the byte stream.
    let mut b = PacketBuilder::new(1);
    b.time(1).time(2).time(3);
    let mut raw = b.finish(1);
    // Corrupt the second message's type byte. First message is 6 bytes.
    raw[UNIT_HEADER_LEN + 6 + 1] = 0xFE;

    let results: Vec<_> = Packet::parse(&raw, Currency::ZAR).unwrap().collect();
    assert_eq!(
        results.len(),
        2,
        "one good message, then the error, then stop"
    );
    assert!(results[0].is_ok());
    assert!(results[1].is_err());
}
