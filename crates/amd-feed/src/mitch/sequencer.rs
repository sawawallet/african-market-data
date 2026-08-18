//! Sequence tracking, A/B arbitration and gap escalation.
//!
//! This is the part that separates a real feed handler from a parser. Decoding
//! is a weekend; deciding what to do when a sequence number jumps is the work.
//!
//! When a gap appears you must assume *some or all* of your books are wrong,
//! and both obvious responses are bad: keep applying messages and you build a
//! book on a transition you never saw, or halt everything and latency explodes
//! across instruments that were never affected. The resolution is a tiered
//! escalation with per-instrument quarantine, so one gap degrades one symbol
//! rather than the venue.
//!
//! ```text
//! Tier 0  A/B arbitration    two identically sequenced feeds; take whichever
//!                            arrives first. Absorbs most single-path loss free.
//! Tier 1  Replay channel     TCP, last 65,000 messages, daily quota per CompID.
//! Tier 2  Snapshot recovery  TCP, full book per segment; buffer live, apply
//!                            snapshot, drain buffer.
//! Tier 3  Exchange reset     Order Book Clear per instrument, then re-published
//!                            Add Orders. On DR failover the sequence resets to 1.
//! ```

use std::collections::HashSet;

/// The Replay channel retransmits from a rolling window of this many messages.
/// A gap wider than this cannot be replayed and must escalate to a snapshot.
pub const REPLAY_WINDOW: u32 = 65_000;

/// Which multicast feed a packet arrived on. The two carry identical sequences.
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
    /// Gap small enough for the Replay channel.
    RequestReplay { from: u32, to: u32 },
    /// Gap wider than the replay window, or replay already exhausted.
    RequestSnapshot { missing_from: u32 },
    /// Sequence went backwards to 1: the feed failed over to the backup site.
    ///
    /// A naive detector reads this as a catastrophic backwards gap and fires
    /// full recovery on every instrument at once — precisely when the exchange
    /// is already degraded. It is an expected transition, handled as its own
    /// case.
    ExchangeRestart,
    /// A heartbeat. Carries no payload and does not advance the sequence.
    Heartbeat,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Stats {
    pub applied: u64,
    pub duplicates: u64,
    pub gaps: u64,
    pub restarts: u64,
    pub replay_requests: u32,
    pub snapshot_requests: u32,
}

/// Per-market-data-group sequence state.
#[derive(Debug)]
pub struct Sequencer {
    /// Sequence the next packet should carry. `None` until the first packet.
    expected: Option<u32>,
    /// Highest sequence delivered by each line, indexed by [`Line::index`].
    ///
    /// This is what disambiguates a duplicate from a restart, and the two are
    /// otherwise genuinely indistinguishable: sequence 1 arriving when we
    /// expect 3 could be the slower line catching up, or the feed failing over
    /// to the backup site. The invariant that separates them is that **a single
    /// line never goes backwards except on restart**. A line delivering a
    /// sequence below its own high-water mark has restarted; a line delivering
    /// a sequence below the *global* expectation but above its own mark is
    /// simply behind its twin.
    line_high: [Option<u32>; 2],
    /// Instruments whose book is not trustworthy pending recovery.
    quarantined: HashSet<u32>,
    /// Replay requests already spent. The server enforces a daily quota per
    /// CompID, and a reconnect loop can burn it in minutes — leaving no
    /// recovery path for the rest of the session. So we govern ourselves
    /// rather than trusting the server to protect us.
    replay_budget: u32,
    stats: Stats,
}

impl Sequencer {
    /// `replay_budget` should be set below whatever the venue actually allows,
    /// leaving headroom for a genuine incident later in the session.
    pub fn new(replay_budget: u32) -> Self {
        Sequencer {
            expected: None,
            line_high: [None, None],
            quarantined: HashSet::new(),
            replay_budget,
            stats: Stats::default(),
        }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    pub fn expected(&self) -> Option<u32> {
        self.expected
    }

    /// Classify a packet by its header sequence, message count and arrival line.
    pub fn observe(&mut self, sequence: u32, message_count: u8, line: Line) -> Action {
        // Heartbeats exercise the line during inactivity and carry no payload,
        // so they neither advance the sequence nor count toward any statistic.
        if message_count == 0 {
            return Action::Heartbeat;
        }

        let last_seq = sequence.wrapping_add(message_count as u32).wrapping_sub(1);
        let line_high = &mut self.line_high[line.index()];

        // A line moving backwards has restarted. This check precedes every
        // other, because a restart looks like a duplicate from the global
        // expectation alone and acting on the wrong one is expensive in both
        // directions: treat a restart as a duplicate and the feed goes silent,
        // treat a duplicate as a restart and you rebuild every book for nothing.
        let line_went_backwards = line_high.is_some_and(|high| sequence <= high);
        if line_went_backwards {
            self.stats.restarts += 1;
            self.expected = Some(sequence.wrapping_add(message_count as u32));
            *line_high = Some(last_seq);
            self.line_high[1 - line.index()] = None;
            self.quarantined.clear();
            return Action::ExchangeRestart;
        }
        *line_high = Some(last_seq);

        let Some(expected) = self.expected else {
            // First packet seen. Trust it and synchronise; there is nothing to
            // compare against yet. Joining mid-session is normal.
            self.expected = Some(sequence.wrapping_add(message_count as u32));
            self.stats.applied += 1;
            return Action::Apply;
        };

        if sequence == expected {
            self.expected = Some(sequence.wrapping_add(message_count as u32));
            self.stats.applied += 1;
            return Action::Apply;
        }

        if sequence < expected {
            // Below the global expectation but not below this line's own mark:
            // the twin already delivered it. This is Tier 0 doing its job.
            self.stats.duplicates += 1;
            return Action::Duplicate;
        }

        // sequence > expected: messages were lost.
        self.stats.gaps += 1;
        let missing = sequence - expected;
        if missing <= REPLAY_WINDOW && self.replay_budget > 0 {
            self.replay_budget -= 1;
            self.stats.replay_requests += 1;
            // Do not advance `expected`: it advances when the replayed
            // messages actually arrive.
            Action::RequestReplay {
                from: expected,
                to: sequence - 1,
            }
        } else {
            self.stats.snapshot_requests += 1;
            // A snapshot resynchronises wholesale, so accept the new position.
            self.expected = Some(sequence.wrapping_add(message_count as u32));
            Action::RequestSnapshot {
                missing_from: expected,
            }
        }
    }

    /// Mark an instrument's book untrustworthy pending recovery.
    pub fn quarantine(&mut self, instrument_id: u32) {
        self.quarantined.insert(instrument_id);
    }

    /// Release an instrument once its book has been rebuilt.
    pub fn release(&mut self, instrument_id: u32) {
        self.quarantined.remove(&instrument_id);
    }

    /// Whether this instrument may be published downstream.
    ///
    /// The whole point of per-instrument quarantine: unaffected symbols keep
    /// flowing while the affected ones recover.
    pub fn is_publishable(&self, instrument_id: u32) -> bool {
        !self.quarantined.contains(&instrument_id)
    }

    pub fn quarantined_count(&self) -> usize {
        self.quarantined.len()
    }

    /// Resynchronise after a completed replay or snapshot.
    ///
    /// Clears the per-line high-water marks: recovered messages did not arrive
    /// on either multicast line, so leaving the marks in place would make the
    /// next live packet look like a backwards jump.
    pub fn resync(&mut self, next_expected: u32) {
        self.expected = Some(next_expected);
        self.line_high = [None, None];
    }
}
