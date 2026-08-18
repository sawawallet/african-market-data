//! The order pool.
//!
//! MITCH is a market-by-order feed: every add, modify, delete and execution is
//! keyed by an order reference, and most of those messages carry **no
//! instrument identifier at all**. `Order Modified` and `Order Executed` name
//! only the order id, so without a pool mapping id back to instrument, side and
//! price, those messages cannot be applied to anything.
//!
//! That is also the thing a direct feed gives you that no quote API can, so the
//! pool is not an implementation detail — it is the reason for taking the feed.

use std::collections::HashMap;

use crate::mitch::Side;

/// A live displayed order, as the book currently understands it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Order {
    pub instrument_id: u32,
    pub side: Side,
    /// Price in minor units at [`crate::mitch::PRICE_SCALE`].
    pub price: i128,
    /// Displayed quantity remaining.
    pub quantity: u32,
}

#[derive(Debug, Default)]
pub struct OrderPool {
    orders: HashMap<u64, Order>,
}

impl OrderPool {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, order_id: u64, order: Order) -> Option<Order> {
        self.orders.insert(order_id, order)
    }

    pub fn get(&self, order_id: u64) -> Option<Order> {
        self.orders.get(&order_id).copied()
    }

    pub fn remove(&mut self, order_id: u64) -> Option<Order> {
        self.orders.remove(&order_id)
    }

    pub fn get_mut(&mut self, order_id: u64) -> Option<&mut Order> {
        self.orders.get_mut(&order_id)
    }

    pub fn len(&self) -> usize {
        self.orders.len()
    }

    pub fn is_empty(&self) -> bool {
        self.orders.is_empty()
    }

    /// Drop every order for one instrument, for `Order Book Clear`.
    pub fn clear_instrument(&mut self, instrument_id: u32) {
        self.orders.retain(|_, o| o.instrument_id != instrument_id);
    }

    pub fn clear(&mut self) {
        self.orders.clear();
    }
}
