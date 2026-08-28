//! Direct exchange feed handlers.
//!
//! This crate is the home for handlers that connect to an exchange's own
//! market data gateway rather than polling a public source. Two protocol
//! families cover the major African venues:
//!
//! - **MITCH** (MillenniumIT) — JSE, NSX, NSE Kenya. UDP multicast delivered as
//!   two identically sequenced feeds, A and B, with TCP replay and recovery
//!   channels alongside. An 8-byte unit header prefixes every packet and only
//!   the first message carries an explicit sequence number; the rest are
//!   implied, so the next packet's expected sequence is `seq + message_count`.
//!   The LSE publishes the same protocol openly as MIT303.
//!
//! - **ITCH over MoldUDP64** (Nasdaq X-Stream) — NGX, which runs X-Stream with
//!   the X-Gen market database and publishes a FIX 5.0 specification. The
//!   transport is implemented in [`moldudp64`]; the message dictionary layered
//!   on top is venue-specific and still needs confirmation against NGX's own
//!   specification.
//!
//! Both are length-prefixed binary framed over UDP multicast with a TCP
//! recovery path, so the intended split is one transport core — A/B
//! arbitration, sequence tracking, per-instrument quarantine, replay quota
//! governor, snapshot-with-buffering — and two message dictionaries.
//!
//! ## What is here, and what is not
//!
//! [`mitch`] is a complete codec — unit header, the book-building message set,
//! implied sequencing, and the tiered gap-recovery state machine — built
//! entirely from the **public** LSE MIT303 specification. It can be developed
//! and tested against synthetic packets without any agreement in place, which
//! is the whole point: the hard part is testable before the paperwork clears.
//!
//! [`moldudp64`] is the NGX side of that same bet: the complete transport —
//! downstream header, length-prefixed blocks, implied sequencing, session
//! tracking and gap escalation — built from the **public** Nasdaq
//! specification. It stops at the transport boundary and hands each block back
//! as raw bytes, because the ITCH dictionary NGX publishes comes from a
//! document distributed under agreement.
//!
//! What is *not* here, and will not be: venue-specific message extensions taken
//! from documents distributed under agreement, connection credentials, and
//! multicast group configuration. Receiving a live feed requires a market data
//! agreement with each venue; an operator who holds one supplies those and
//! registers the resulting handler at runtime.
//!
//! The rest of the workspace is built to receive that data unchanged:
//! [`amd_core::DEFAULT_SCALE`] is 8 so MITCH prices land without rescaling, and
//! [`amd_core::Provenance`] carries `sequence` and `recovered` because a feed
//! handler has both and a REST source has neither.

pub mod book;
pub mod mitch;
pub mod moldudp64;
