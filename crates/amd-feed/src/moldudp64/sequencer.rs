//! Sequence tracking, A/B arbitration and gap escalation for MoldUDP64.
//!
//! The shape mirrors the MITCH sequencer deliberately — same tiering, same
//! per-instrument degradation philosophy — but three protocol differences
//! change the decisions:
//!
//! - **Restart is a session change, not a sequence reset.** MITCH failover
//!   drops the sequence back to 1; MoldUDP64 issues a new session id and starts
//!   again at 1 within it. A handler watching only the sequence cannot tell a
//!   new session from a catastrophic backwards gap, and will either rebuild
//!   needlessly or, worse, keep applying against a stale book.
//! - **Retransmission is a request/response over unicast UDP**, not a TCP
//!   replay channel, and the request itself carries a message count — so the
//!   gap is bounded by what one request may ask for rather than by a rolling
//!   window.
//! - **The sequence is a `u64`.** It will not wrap. Anything that looks like a
//!   wrap is a desync and is treated as one.

use super::header::DownstreamHeader;
use super::types::Session;

/// The largest run of messages a single retransmission request may ask for.
///
/// A request names a starting sequence and a count, and the count is a `u16`.
/// A gap wider than this cannot be closed by one request; rather than firing a
/// storm of them at an exchange that is already dropping packets, escalate.
pub const MAX_RETRANSMIT: u64 = u16::MAX as u64;

/// Which feed a packet arrived on. X-Stream deployments publish two identically
/// sequenced multicast groups; the protocol itself is indifferent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    A,
    B,
}

impl Line {
    #[inline]
    fn index(self) -> usize {
        match self {
            Line::A => 0,
            Line::B => 1,
        }
    }
}

/// What the handler should do with a packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// In sequence. Apply it.
    Apply,
    /// Already seen on the other line. Discard without applying.
    Duplicate,
    /// Gap small enough for one retransmission request.
    RequestRetransmit { from: u64, count: u16 },
    /// Gap too wide for a single request, or retransmission already exhausted.
    RequestSnapshot { missing_from: u64 },
    /// A different session id. Everything built so far belongs to the old one:
    /// discard the books and rebuild.
    SessionChanged { from: Session, to: Session },
    /// Carries no payload and does not advance the sequence.
    Heartbeat,
    /// The venue has closed the session. No further packets are expected.
    EndOfSession,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub applied: u64,
    pub duplicates: u64,
    pub gaps: u64,
    pub session_changes: u64,
    pub retransmit_requests: u32,
    pub snapshot_requests: u32,
}

/// Tracks one instrument segment across both lines.
#[derive(Debug)]
pub struct Sequencer {
    session: Option<Session>,
    /// Sequence the next in-order packet should carry.
    expected: u64,
    /// Highest `next_expected` seen per line, for A/B arbitration.
    seen: [u64; 2],
    stats: Stats,
}

impl Default for Sequencer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sequencer {
    pub fn new() -> Self {
        Sequencer {
            session: None,
            expected: 0,
            seen: [0, 0],
            stats: Stats::default(),
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn expected(&self) -> u64 {
        self.expected
    }

    pub fn session(&self) -> Option<Session> {
        self.session
    }

    /// Classify a packet.
    ///
    /// Order matters here. Session is checked before sequence, because a new
    /// session legitimately restarts at 1 and every sequence-based test would
    /// misread that as the feed jumping backwards.
    pub fn observe(&mut self, header: DownstreamHeader, line: Line) -> Action {
        match self.session {
            None => {
                self.session = Some(header.session);
                self.expected = header.sequence;
            }
            Some(current) if current != header.session => {
                let previous = current;
                self.session = Some(header.session);
                self.expected = header.sequence;
                self.seen = [0, 0];
                self.stats.session_changes += 1;
                return Action::SessionChanged {
                    from: previous,
                    to: header.session,
                };
            }
            Some(_) => {}
        }

        if header.is_end_of_session() {
            return Action::EndOfSession;
        }
        if header.is_heartbeat() {
            return Action::Heartbeat;
        }

        let next = header.next_expected();
        self.seen[line.index()] = self.seen[line.index()].max(next);

        // Wholly behind what we have already applied: the other line beat it.
        if next <= self.expected {
            self.stats.duplicates += 1;
            return Action::Duplicate;
        }

        if header.sequence > self.expected {
            let missing = header.sequence - self.expected;
            self.stats.gaps += 1;
            return if missing <= MAX_RETRANSMIT {
                self.stats.retransmit_requests += 1;
                Action::RequestRetransmit {
                    from: self.expected,
                    count: missing as u16,
                }
            } else {
                self.stats.snapshot_requests += 1;
                Action::RequestSnapshot {
                    missing_from: self.expected,
                }
            };
        }

        // Starts at or before what we expect and ends beyond it: it overlaps.
        // Applying is correct; the overlapping prefix is a replayed duplicate
        // the caller filters by per-message sequence.
        self.expected = next;
        self.stats.applied += 1;
        Action::Apply
    }

    /// Confirm a gap has been closed, so the next packet is judged from here.
    pub fn recovered_to(&mut self, sequence: u64) {
        self.expected = self.expected.max(sequence);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moldudp64::header::END_OF_SESSION;

    fn hdr(session: &str, seq: u64, count: u16) -> DownstreamHeader {
        let mut buf = format!("{session:<10}").into_bytes();
        buf.extend_from_slice(&seq.to_be_bytes());
        buf.extend_from_slice(&count.to_be_bytes());
        DownstreamHeader::decode(&buf).unwrap()
    }

    #[test]
    fn in_order_packets_apply_and_advance() {
        let mut s = Sequencer::new();
        assert_eq!(s.observe(hdr("S1", 1, 2), Line::A), Action::Apply);
        assert_eq!(s.expected(), 3);
        assert_eq!(s.observe(hdr("S1", 3, 1), Line::A), Action::Apply);
        assert_eq!(s.expected(), 4);
    }

    #[test]
    fn the_second_line_is_a_duplicate_not_a_gap() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 2), Line::A);
        assert_eq!(s.observe(hdr("S1", 1, 2), Line::B), Action::Duplicate);
        assert_eq!(s.stats().gaps, 0, "A/B arbitration must not look like loss");
    }

    #[test]
    fn a_small_gap_asks_for_retransmission() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 1), Line::A); // expected -> 2
        assert_eq!(
            s.observe(hdr("S1", 10, 1), Line::A),
            Action::RequestRetransmit { from: 2, count: 8 }
        );
    }

    #[test]
    fn a_gap_wider_than_one_request_escalates_to_a_snapshot() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 1), Line::A);
        let far = 2 + MAX_RETRANSMIT + 1;
        assert_eq!(
            s.observe(hdr("S1", far, 1), Line::A),
            Action::RequestSnapshot { missing_from: 2 }
        );
    }

    #[test]
    fn a_new_session_is_a_restart_not_a_backwards_gap() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 500, 1), Line::A);
        let action = s.observe(hdr("S2", 1, 1), Line::A);
        match action {
            Action::SessionChanged { from, to } => {
                assert_eq!(from.as_str(), "S1");
                assert_eq!(to.as_str(), "S2");
            }
            other => panic!("expected a session change, got {other:?}"),
        }
        assert_eq!(s.expected(), 1, "the new session restarts the count");
        assert_eq!(s.stats().gaps, 0, "a restart is not packet loss");
    }

    #[test]
    fn heartbeats_do_not_advance_or_count_as_gaps() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 2), Line::A);
        let before = s.expected();
        assert_eq!(s.observe(hdr("S1", 3, 0), Line::A), Action::Heartbeat);
        assert_eq!(s.expected(), before);
        assert_eq!(s.stats().gaps, 0);
    }

    #[test]
    fn end_of_session_is_reported_not_treated_as_65535_messages() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 1), Line::A);
        assert_eq!(
            s.observe(hdr("S1", 2, END_OF_SESSION), Line::A),
            Action::EndOfSession
        );
        assert_eq!(
            s.expected(),
            2,
            "the sentinel must not advance the sequence"
        );
    }

    #[test]
    fn recovery_closes_the_gap() {
        let mut s = Sequencer::new();
        s.observe(hdr("S1", 1, 1), Line::A);
        s.observe(hdr("S1", 10, 1), Line::A); // gap
        s.recovered_to(10);
        assert_eq!(s.observe(hdr("S1", 10, 1), Line::A), Action::Apply);
    }
}
