//! Gap detection and recovery escalation.
//!
//! Most feed handlers are correct on the happy path and wrong here, so these
//! scenarios were written before any production data path existed.

use amd_feed::mitch::{Action, Line, PacketBuilder, REPLAY_WINDOW, Sequencer};

/// Feed the sequencer a run of in-order packets, one message each.
fn in_order(seq: &mut Sequencer, from: u32, count: u32) {
    for n in 0..count {
        assert_eq!(seq.observe(from + n, 1, Line::A), Action::Apply);
    }
}

#[test]
fn the_first_packet_synchronises_rather_than_reporting_a_gap() {
    let mut s = Sequencer::new(10);
    assert_eq!(s.expected(), None);
    // Joining mid-session at sequence 918_273 is normal, not a 918,273-message
    // gap. There is nothing to compare against yet.
    assert_eq!(s.observe(918_273, 4, Line::A), Action::Apply);
    assert_eq!(s.expected(), Some(918_277));
    assert_eq!(s.stats().gaps, 0);
}

#[test]
fn in_order_packets_apply_and_advance() {
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 5);
    assert_eq!(s.expected(), Some(6));
    assert_eq!(s.stats().applied, 5);
    assert_eq!(s.stats().gaps, 0);
}

#[test]
fn heartbeats_do_not_advance_the_sequence() {
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 3);
    let before = s.expected();
    assert_eq!(s.observe(4, 0, Line::A), Action::Heartbeat);
    assert_eq!(s.expected(), before, "a heartbeat carries no payload");
    // The next real packet still lands in sequence.
    assert_eq!(s.observe(4, 1, Line::A), Action::Apply);
}

#[test]
fn the_second_line_is_arbitrated_away_as_duplicate() {
    // Tier 0. Two identically sequenced feeds; whichever arrives second is
    // already applied. This absorbs most single-path loss at no cost.
    let mut s = Sequencer::new(10);
    assert_eq!(s.observe(1, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(2, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(1, 1, Line::B), Action::Duplicate);
    assert_eq!(s.observe(2, 1, Line::B), Action::Duplicate);
    assert_eq!(s.stats().applied, 2);
    assert_eq!(s.stats().duplicates, 2);
    assert_eq!(s.stats().gaps, 0);
}

#[test]
fn a_small_gap_escalates_to_replay() {
    // Tier 1.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 3); // expecting 4
    assert_eq!(
        s.observe(9, 1, Line::A),
        Action::RequestReplay { from: 4, to: 8 }
    );
    assert_eq!(s.stats().gaps, 1);
    assert_eq!(s.stats().replay_requests, 1);
    // `expected` must not advance: it advances when the replayed messages
    // actually arrive, not when they are asked for.
    assert_eq!(s.expected(), Some(4));
}

#[test]
fn a_gap_wider_than_the_replay_window_goes_straight_to_snapshot() {
    // Tier 2. The Replay channel holds only the last 65,000 messages, so a
    // wider gap cannot be served by it at all.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 1); // expecting 2
    let far = 2 + REPLAY_WINDOW + 1;
    assert_eq!(
        s.observe(far, 1, Line::A),
        Action::RequestSnapshot { missing_from: 2 }
    );
    assert_eq!(s.stats().replay_requests, 0);
    assert_eq!(s.stats().snapshot_requests, 1);
}

#[test]
fn replay_budget_exhaustion_falls_back_to_snapshot() {
    // The server enforces a daily quota per CompID. A reconnect loop can burn
    // it in minutes and leave no recovery path for the rest of the session, so
    // the handler governs itself rather than trusting the server to.
    let mut s = Sequencer::new(2);
    let mut at = 1;
    for _ in 0..2 {
        assert_eq!(s.observe(at, 1, Line::A), Action::Apply);
        at += 10;
        assert!(matches!(
            s.observe(at, 1, Line::A),
            Action::RequestReplay { .. }
        ));
        s.resync(at + 1);
        at += 1;
    }
    assert_eq!(s.stats().replay_requests, 2);

    // Budget spent: the next gap must escalate rather than silently retry.
    at += 10;
    assert!(matches!(
        s.observe(at, 1, Line::A),
        Action::RequestSnapshot { .. }
    ));
    assert_eq!(s.stats().replay_requests, 2, "budget must not be exceeded");
    assert_eq!(s.stats().snapshot_requests, 1);
}

#[test]
fn a_reset_to_one_is_a_failover_not_a_catastrophic_gap() {
    // Tier 3, and the trap. On failover to the backup site the multicast
    // sequence resets to 1. A naive detector reads that as an enormous
    // backwards jump and fires full recovery on every instrument at once —
    // exactly when the exchange is already degraded.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 50_000);
    assert_eq!(s.expected(), Some(50_001));

    s.quarantine(42);
    s.quarantine(43);
    assert_eq!(s.quarantined_count(), 2);

    assert_eq!(s.observe(1, 1, Line::A), Action::ExchangeRestart);
    assert_eq!(s.stats().restarts, 1);
    assert_eq!(
        s.stats().snapshot_requests,
        0,
        "a restart is not a snapshot request"
    );
    assert_eq!(s.expected(), Some(2), "resynchronised to the new stream");
    assert_eq!(
        s.quarantined_count(),
        0,
        "the old book is gone; quarantine is moot"
    );
}

#[test]
fn quarantine_is_per_instrument_so_one_gap_does_not_stall_the_venue() {
    // The whole reason for per-instrument quarantine: unaffected symbols keep
    // flowing while the affected ones recover.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 10);

    s.quarantine(100);
    assert!(!s.is_publishable(100));
    assert!(
        s.is_publishable(101),
        "an unaffected instrument must keep publishing"
    );

    s.release(100);
    assert!(s.is_publishable(100));
    assert_eq!(s.quarantined_count(), 0);
}

#[test]
fn resync_after_recovery_restores_the_happy_path() {
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 3);
    assert!(matches!(
        s.observe(20, 1, Line::A),
        Action::RequestReplay { from: 4, to: 19 }
    ));

    // Replay delivered 4..=19; the live packet at 20 is now next.
    s.resync(20);
    assert_eq!(s.observe(20, 1, Line::A), Action::Apply);
    assert_eq!(s.expected(), Some(21));
}

#[test]
fn a_realistic_session_survives_loss_reorder_and_failover() {
    // Composite scenario. Every branch of the state machine, in the order a
    // real morning tends to produce them.
    let mut s = Sequencer::new(5);

    in_order(&mut s, 1, 100); // clean open
    assert_eq!(s.observe(101, 0, Line::A), Action::Heartbeat); // quiet spell
    assert_eq!(s.observe(50, 1, Line::B), Action::Duplicate); // B arrives late

    // Brief loss, replayed.
    assert!(matches!(
        s.observe(105, 2, Line::A),
        Action::RequestReplay { from: 101, to: 104 }
    ));
    s.resync(107);
    assert_eq!(s.observe(107, 1, Line::A), Action::Apply);

    // Sustained outage past the replay window.
    let far = 108 + REPLAY_WINDOW + 500;
    assert!(matches!(
        s.observe(far, 1, Line::A),
        Action::RequestSnapshot { .. }
    ));

    // Then the exchange fails over.
    assert_eq!(s.observe(1, 1, Line::A), Action::ExchangeRestart);
    in_order(&mut s, 2, 10);

    let stats = s.stats();
    assert_eq!(stats.restarts, 1);
    assert_eq!(stats.gaps, 2);
    assert_eq!(stats.replay_requests, 1);
    assert_eq!(stats.snapshot_requests, 1);
    assert_eq!(stats.duplicates, 1);
}

#[test]
fn a_built_packet_feeds_the_sequencer_end_to_end() {
    // Ties the builder, the header and the sequencer together, so the three
    // cannot drift apart.
    let mut s = Sequencer::new(10);
    let mut expected = 1u32;

    for round in 0..20u32 {
        let mut b = PacketBuilder::new(1);
        b.time(34_200 + round)
            .order_book_clear(round, 7, amd_feed::mitch::Flags(0));
        let raw = b.finish(expected);

        let packet = amd_feed::mitch::Packet::parse(&raw, amd_core::Currency::ZAR).unwrap();
        let header = packet.header();
        assert_eq!(
            s.observe(header.sequence, header.message_count, Line::A),
            Action::Apply
        );

        let seqs: Vec<u32> = packet.map(|m| m.unwrap().sequence).collect();
        assert_eq!(seqs, vec![expected, expected + 1]);
        expected = header.next_expected();
    }

    assert_eq!(s.expected(), Some(expected));
    assert_eq!(s.stats().gaps, 0);
}

#[test]
fn a_low_sequence_is_disambiguated_by_which_line_carried_it() {
    // The subtlest case in the crate. Sequence 1 arriving when we expect 3 is
    // *either* the slower multicast line catching up, or the feed failing over
    // to the backup site — and from the global expectation alone the two are
    // indistinguishable. The separating invariant is that a single line never
    // goes backwards except on restart.

    // Line B trailing its twin is a duplicate, however far back it is.
    let mut s = Sequencer::new(10);
    assert_eq!(s.observe(1, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(2, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(1, 1, Line::B), Action::Duplicate);
    assert_eq!(s.stats().restarts, 0, "a trailing line is not a restart");

    // The same line going backwards is a restart, even by one message.
    let mut s = Sequencer::new(10);
    assert_eq!(s.observe(1, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(2, 1, Line::A), Action::Apply);
    assert_eq!(s.observe(1, 1, Line::A), Action::ExchangeRestart);
    assert_eq!(s.stats().duplicates, 0, "a rewound line is not a duplicate");
}

#[test]
fn a_restart_resets_both_lines_so_the_twin_is_not_read_as_rewound() {
    // After a failover, the other line also restarts. Its first post-restart
    // packet must read as normal traffic, not as a second restart.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 100);
    assert_eq!(s.observe(50, 1, Line::B), Action::Duplicate);

    assert_eq!(s.observe(1, 1, Line::A), Action::ExchangeRestart);
    assert_eq!(s.observe(2, 1, Line::A), Action::Apply);
    // Line B now delivers the restarted stream too.
    assert_eq!(s.observe(1, 1, Line::B), Action::Duplicate);
    assert_eq!(s.stats().restarts, 1, "exactly one restart, not two");
}

#[test]
fn resync_clears_line_marks_so_live_traffic_is_not_read_as_rewound() {
    // Recovered messages arrive over TCP, not on either multicast line. If the
    // marks survived, the next live packet would look like a backwards jump.
    let mut s = Sequencer::new(10);
    in_order(&mut s, 1, 5);
    assert!(matches!(
        s.observe(100, 1, Line::A),
        Action::RequestReplay { .. }
    ));

    s.resync(6);
    // Replay delivered 6..=99 over TCP; live multicast resumes at 6.
    assert_eq!(s.observe(6, 1, Line::A), Action::Apply);
    assert_eq!(s.stats().restarts, 0);
}
