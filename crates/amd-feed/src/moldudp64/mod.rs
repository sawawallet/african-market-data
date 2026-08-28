//! MoldUDP64 transport, built from the public Nasdaq specification.
//!
//! MoldUDP64 is the framing and recovery layer under Nasdaq's market data
//! protocols. NGX runs X-Stream with the X-Gen market database, which is the
//! Nasdaq stack, so this is the transport a direct NGX feed arrives on.
//!
//! It exists here for the same reason the MITCH codec does: the specification
//! is published openly, so the hard part — framing, implied sequencing, gap
//! escalation, A/B arbitration — can be written and tested against synthetic
//! packets long before any market data agreement is signed.
//!
//! Transport shape:
//!
//! - **Downstream** — UDP multicast. A 20-byte header per datagram, then
//!   length-prefixed message blocks. The header numbers only the first block;
//!   the rest are implied.
//! - **Retransmission** — a unicast UDP request naming a starting sequence and
//!   a count, answered by the rerequest server.
//! - **Session** — a ten-byte id, constant for the trading day. It changing is
//!   how a restart is signalled.
//!
//! ## What is here, and what is not
//!
//! This module is the transport only. It hands each message block back as raw
//! bytes and does not interpret them, because the message dictionary NGX
//! publishes comes from a specification distributed under agreement. That
//! dictionary, connection credentials and multicast group configuration are
//! supplied by an operator who holds a market data agreement and registered at
//! runtime — exactly as [`crate::mitch`] treats venue-specific extensions.
//!
//! The seam is [`packet::Sequenced::payload`]: give it to your decoder.
//!
//! ```no_run
//! use amd_feed::moldudp64::{Packet, Sequencer, Line, Action};
//!
//! let mut seq = Sequencer::new();
//! # let datagram: &[u8] = &[];
//! let packet = Packet::parse(datagram)?;
//!
//! match seq.observe(packet.header(), Line::A) {
//!     Action::Apply => {
//!         for block in packet {
//!             let block = block?;
//!             // block.payload -> the operator's ITCH decoder
//!             let _ = (block.sequence, block.payload);
//!         }
//!     }
//!     Action::Duplicate => {}                          // the other line won
//!     Action::SessionChanged { .. } => {}              // rebuild from snapshot
//!     Action::RequestRetransmit { from, count } => {}  // small gap
//!     Action::RequestSnapshot { missing_from } => {}   // too wide to request
//!     Action::Heartbeat => {}
//!     Action::EndOfSession => {}
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod header;
pub mod packet;
pub mod sequencer;
pub mod types;

pub use header::{DOWNSTREAM_HEADER_LEN, DownstreamHeader, END_OF_SESSION};
pub use packet::{Packet, Sequenced};
pub use sequencer::{Action, Line, MAX_RETRANSMIT, Sequencer, Stats};
pub use types::{DecodeError, SESSION_LEN, Session};
