//! Direct exchange feed handlers. **Intentionally empty in this repository.**
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
//!   the X-Gen market database and publishes a FIX 5.0 specification. The exact
//!   native encoding is inferred from X-Stream deployments elsewhere and needs
//!   confirmation against NGX's own specification.
//!
//! Both are length-prefixed binary framed over UDP multicast with a TCP
//! recovery path, so the intended split is one transport core — A/B
//! arbitration, sequence tracking, per-instrument quarantine, replay quota
//! governor, snapshot-with-buffering — and two message dictionaries.
//!
//! ## Why there is no code here
//!
//! Receiving these feeds requires a market data agreement with each venue, and
//! the specifications are distributed under terms that do not permit
//! redistribution. An operator who holds the licence implements against those
//! documents and registers the handler at runtime through
//! [`amd_adapters::Adapter`], which is public for exactly this reason.
//!
//! The rest of the workspace is built to receive that data unchanged:
//! [`amd_core::DEFAULT_SCALE`] is 8 so MITCH prices land without rescaling, and
//! [`amd_core::Provenance`] carries `sequence` and `recovered` because a feed
//! handler has both and a REST source has neither.
