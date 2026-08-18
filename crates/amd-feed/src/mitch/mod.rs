//! MITCH codec, built from the public LSE MIT303 Level 2 specification.
//!
//! MITCH is the MillenniumIT market data protocol. JSE, NSX and NSE Kenya all
//! run MillenniumIT matching engines, so one codec covers three venues — and
//! the LSE publishes the same protocol openly, which is why this code can exist
//! in a public repository while venue-specific extensions cannot.
//!
//! Transport shape:
//!
//! - **Real-Time** — UDP/IPv4 multicast, delivered as two identically sequenced
//!   feeds, A and B. One unit header per packet, always.
//! - **Replay** — TCP. Retransmits from a rolling window of the last 65,000
//!   messages, under a daily quota per CompID.
//! - **Recovery** — TCP. Full order book snapshot per segment.
//!
//! ```no_run
//! use amd_core::Currency;
//! use amd_feed::mitch::{Packet, Sequencer, Line, Action};
//!
//! let mut seq = Sequencer::new(20);
//! # let datagram: &[u8] = &[];
//! let packet = Packet::parse(datagram, Currency::ZAR)?;
//! let header = packet.header();
//!
//! match seq.observe(header.sequence, header.message_count, Line::A) {
//!     Action::Apply => {
//!         for message in packet {
//!             let _ = message?;
//!         }
//!     }
//!     Action::Duplicate => {}                        // arrived on the other line first
//!     Action::ExchangeRestart => {}                  // failover; rebuild from snapshot
//!     Action::RequestReplay { from, to } => {}       // small gap
//!     Action::RequestSnapshot { missing_from } => {} // gap past the replay window
//!     Action::Heartbeat => {}
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod build;
pub mod header;
pub mod message;
pub mod packet;
pub mod sequencer;
pub mod types;

pub use build::PacketBuilder;
pub use header::{UNIT_HEADER_LEN, UnitHeader};
pub use message::{Message, msg_type};
pub use packet::{Packet, Sequenced};
pub use sequencer::{Action, Line, REPLAY_WINDOW, Sequencer, Stats};
pub use types::{DecodeError, Flags, PRICE_SCALE, Side, SystemEventCode};
