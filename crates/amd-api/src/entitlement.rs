//! Egress-boundary entitlement filtering.
//!
//! This is the last place a datum can be stopped before it leaves the system,
//! and therefore the only place the check is trustworthy. It runs against the
//! [`DelayClass`] carried on the quote itself rather than against the route,
//! the adapter or the caller's expectations — by the time a quote reaches here
//! it may have arrived through any of several adapters holding different
//! entitlements, and only the datum knows which.

use amd_core::{DelayClass, Quote};
use amd_store::ApiKey;

/// What was withheld, so the response can say so rather than silently
/// returning a shorter list.
#[derive(Debug, Default, Clone, Copy, serde::Serialize)]
pub struct Withheld {
    pub count: usize,
    /// The strictest class the caller would have needed. `None` when nothing
    /// was withheld.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires: Option<&'static str>,
}

/// Anonymous callers get end-of-day only.
///
/// Deliberately not `Unknown`: `Unknown` means "the source makes no claim",
/// which is a different statement from "you are entitled to EOD". Sources that
/// report `Unknown` are served to anonymous callers because we cannot prove
/// they are licensed data, and withholding them would make the free tier
/// useless without protecting anything.
pub const ANONYMOUS_MAX: DelayClass = DelayClass::Eod;

/// Filter quotes to what this caller may receive.
pub fn filter(quotes: Vec<Quote>, key: Option<&ApiKey>) -> (Vec<Quote>, Withheld) {
    let mut withheld = Withheld::default();
    let mut strictest: Option<DelayClass> = None;

    let kept = quotes
        .into_iter()
        .filter(|q| {
            let delay = q.provenance.delay;
            let allowed = match key {
                Some(k) => k.may_receive(q.instrument.exchange, delay),
                // Unknown-class data is not licensed data, so it passes.
                None => delay == DelayClass::Unknown || ANONYMOUS_MAX.permits(delay),
            };
            if !allowed {
                withheld.count += 1;
                strictest = Some(strictest.map_or(delay, |s: DelayClass| s.min(delay)));
            }
            allowed
        })
        .collect();

    withheld.requires = strictest.map(DelayClass::as_str);
    (kept, withheld)
}
