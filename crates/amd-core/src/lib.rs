//! Core domain types for African market data.
//!
//! Two invariants govern this crate, and both exist because breaking them fails
//! silently rather than loudly:
//!
//! 1. **Prices are fixed-point integers, never floats.** See [`money`].
//! 2. **Every datum carries its provenance.** See [`provenance`].

pub mod instrument;
pub mod money;
pub mod provenance;
pub mod quote;

pub use instrument::{ExchangeCode, Instrument, InstrumentId};
pub use money::{Currency, DEFAULT_SCALE, MoneyError, Price};
pub use provenance::{DelayClass, Provenance};
pub use quote::{Bar, Quote};
