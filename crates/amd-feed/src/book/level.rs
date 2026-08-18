//! Price levels with time priority preserved.
//!
//! Levels keep the queue of order ids, not just an aggregate quantity. Dropping
//! the queue would be cheaper and would still produce correct top-of-book
//! quotes — but queue position is precisely the information a market-by-order
//! feed carries and a quote API does not, so discarding it would throw away the
//! reason for taking the feed in the first place.
//!
//! Prices key the maps as raw `i128` minor units at [`PRICE_SCALE`], because
//! [`amd_core::Price`] is deliberately not `Ord`: comparing two prices can fail
//! on a currency mismatch, and a `BTreeMap` cannot express that. Within one
//! book the currency is fixed and the scale is fixed, so the integer is a
//! total order and the comparison is exact.
//!
//! [`PRICE_SCALE`]: crate::mitch::PRICE_SCALE

use std::collections::VecDeque;

/// One price level: aggregate displayed size plus the queue behind it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Level {
    /// Sum of displayed quantity across the queue.
    pub quantity: u64,
    /// Order ids in time priority, front first.
    pub queue: VecDeque<u64>,
}

impl Level {
    pub fn order_count(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Join the back of the queue — a new order, or one that lost priority.
    pub fn push_back(&mut self, order_id: u64, quantity: u32) {
        self.queue.push_back(order_id);
        self.quantity += quantity as u64;
    }

    /// Leave the queue entirely.
    ///
    /// Returns whether the order was actually present. A `false` here means the
    /// book and the pool disagree, which is worth surfacing rather than
    /// swallowing.
    pub fn remove(&mut self, order_id: u64, quantity: u32) -> bool {
        if let Some(pos) = self.queue.iter().position(|id| *id == order_id) {
            self.queue.remove(pos);
            self.quantity = self.quantity.saturating_sub(quantity as u64);
            true
        } else {
            false
        }
    }

    /// Change an order's size without moving it in the queue.
    ///
    /// Used when a modify retains priority, and after a partial execution —
    /// a fill reduces size but does not cost the remainder its place.
    pub fn resize(&mut self, from: u32, to: u32) {
        self.quantity = self.quantity.saturating_sub(from as u64) + to as u64;
    }
}
