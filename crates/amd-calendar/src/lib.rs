//! Trading sessions, exchange-local dates and staleness.
//!
//! Public holidays are deliberately **not** modelled. A wrong holiday calendar
//! is worse than an absent one, because it silently reports a closed market as
//! open — and holiday schedules across seventeen African venues change yearly
//! and are published inconsistently. [`session_state`] therefore claims only
//! "this is a scheduled trading weekday and the clock is inside a session
//! window", which is exactly what it can prove. Callers needing a hard answer
//! should combine it with an exchange-published holiday feed.

pub mod registry;

use amd_core::ExchangeCode;
use jiff::Timestamp;
use jiff::civil::Weekday;
use jiff::tz::TimeZone;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

pub use registry::{VENUES, Venue, Window, unverified, venue};

#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    #[error("venue {0} declares timezone {1:?}, which is not in the tzdb")]
    UnknownTimezone(ExchangeCode, String),
    #[error("timestamp out of representable range")]
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    /// A scheduled trading day, before the first window opens.
    PreOpen,
    /// Inside a trading window.
    Open,
    /// After the last window closes, or not a scheduled trading day.
    Closed,
}

fn to_jiff(at: OffsetDateTime) -> Result<Timestamp, CalendarError> {
    Timestamp::from_nanosecond(at.unix_timestamp_nanos()).map_err(|_| CalendarError::OutOfRange)
}

fn zone(v: &Venue) -> Result<TimeZone, CalendarError> {
    TimeZone::get(v.timezone)
        .map_err(|_| CalendarError::UnknownTimezone(v.code, v.timezone.to_owned()))
}

/// Monday is bit 0, Sunday is bit 6.
fn weekday_bit(w: Weekday) -> u8 {
    1 << (w.to_monday_zero_offset() as u8)
}

/// Whether `at` falls on a scheduled trading weekday for this venue.
///
/// Weekday only — see the module note on holidays. A `true` result means "not a
/// weekend for this venue", not "the market is definitely trading".
pub fn is_trading_day(at: OffsetDateTime, v: &Venue) -> Result<bool, CalendarError> {
    let zoned = to_jiff(at)?.to_zoned(zone(v)?);
    Ok(v.trading_days & weekday_bit(zoned.weekday()) != 0)
}

/// Current session state. Weekday-accurate and holiday-blind.
pub fn session_state(at: OffsetDateTime, v: &Venue) -> Result<SessionState, CalendarError> {
    let zoned = to_jiff(at)?.to_zoned(zone(v)?);
    if v.trading_days & weekday_bit(zoned.weekday()) == 0 {
        return Ok(SessionState::Closed);
    }
    let now = zoned.hour() as u16 * 60 + zoned.minute() as u16;
    let mut earliest_open = u16::MAX;
    for w in v.sessions {
        if now >= w.open_min && now < w.close_min {
            return Ok(SessionState::Open);
        }
        earliest_open = earliest_open.min(w.open_min);
    }
    Ok(if now < earliest_open {
        SessionState::PreOpen
    } else {
        SessionState::Closed
    })
}

pub fn is_open(at: OffsetDateTime, v: &Venue) -> Result<bool, CalendarError> {
    Ok(session_state(at, v)? == SessionState::Open)
}

/// Session date in the venue's own timezone, as `YYYY-MM-DD`.
///
/// Not the UTC date. At 23:30 UTC it is already tomorrow in Nairobi, and a bar
/// keyed by the UTC date would land in the wrong session.
pub fn session_date(at: OffsetDateTime, v: &Venue) -> Result<String, CalendarError> {
    let zoned = to_jiff(at)?.to_zoned(zone(v)?);
    Ok(format!(
        "{:04}-{:02}-{:02}",
        zoned.year(),
        zoned.month(),
        zoned.day()
    ))
}

/// Seconds since `as_of`, clamped at zero.
///
/// Use this rather than eyeballing a timestamp. On thin venues a price that has
/// not moved in two hours and a price that is two hours stale look identical in
/// a UI, and only this number tells them apart.
pub fn staleness_seconds(as_of: OffsetDateTime, now: OffsetDateTime) -> i64 {
    (now - as_of).whole_seconds().max(0)
}

/// Convenience wrapper resolving the venue from its code.
pub fn state_for(at: OffsetDateTime, code: ExchangeCode) -> Result<SessionState, CalendarError> {
    session_state(at, venue(code))
}
