# Holiday awareness: the boundary, and a feed-shaped interface

Status: design note. Nothing described under "Proposed interface" exists in the
code yet.

## What is modelled today

`amd_calendar::session_state` (and `calendar.ts` in the SDK)
answer one question: is this a scheduled trading weekday for the venue, and is
the exchange-local clock inside a session window? Inputs are the venue's
`trading_days` mask, its `sessions` windows and its IANA timezone. Nothing else.

A `Closed` result is reliable. An `Open` result means "scheduled to be open",
not "is trading": on a public holiday the library will say `Open` during the
window.

## Why holidays are deliberately not modelled

1. **A wrong holiday table fails silently, in the dangerous direction.** A
   missing holiday reports a closed market as open. A spurious holiday reports
   an open market as closed. Neither raises an error, and on thin venues a
   closed market and a quiet one look the same.
2. **There is no single trustworthy source.** Each exchange publishes its own
   calendar (a circular, a web page, a PDF), in its own format, on its own
   schedule. This repository has no verified holiday source for any venue and
   will not copy dates from third-party listings (see CONTRIBUTING).
3. **Dates are not computable.** Several venues close on religious holidays
   tied to lunar calendars and announced by sighting or decree, and ad hoc
   closures (elections, national mourning, system outages) are declared at
   short notice. A static table baked into the registry would go stale every
   year, which is the same failure mode `sessions_verified` exists to prevent.
4. **Provenance matters more than coverage.** The registry's ethos is that
   "unverified" is an honest state. A holiday table needs the same treatment,
   per venue and per year, with a cited source.

## What the API should say instead of guessing

Until a venue has a sourced holiday feed, the calendar answer is "holiday
status unknown", and consumers should be able to see that, instead of reading
`Open` as a guarantee. The existing `unverified()` / `sessions_verified` pattern
is the template: say what is and is not known.

## Proposed interface (not implemented)

Keep the pure function pure and push holiday knowledge in from outside, so the
calendar crate never owns dates it cannot cite.

```rust
/// Where a closure date came from. Always cited; never inferred.
pub struct Closure {
    pub date: civil::Date,        // venue-local date
    pub reason: Option<String>,   // as published, not paraphrased
    pub source: String,           // URL or document reference
    pub published_at: Timestamp,  // when the exchange announced it
}

/// Implemented by whatever supplies closures: a file, an adapter, an API.
pub trait ClosureFeed {
    /// Closures known for `venue` covering `year`, or None if this feed has
    /// no coverage for that venue/year. None is not the same as "no holidays".
    fn closures(&self, venue: ExchangeCode, year: i16) -> Option<Vec<Closure>>;
}

pub enum HolidayStatus {
    /// Feed covers this venue and year; the date is not a closure.
    Clear,
    /// Feed lists this date as a closure.
    Closed(Closure),
    /// No feed coverage. The honest default.
    Unknown,
}

pub fn session_state_with(
    at: OffsetDateTime,
    v: &Venue,
    feed: &dyn ClosureFeed,
) -> Result<(SessionState, HolidayStatus), CalendarError>;
```

Design points:

- `session_state` keeps its current signature and behaviour. Existing callers
  (ingestor, API) are unaffected.
- `Unknown` is distinct from `Clear`. A feed that returns `None` must never be
  treated as "no holidays".
- Coverage is per venue and per year, so a venue can be sourced for 2026 and
  unknown for 2027.
- A `Closed(Closure)` forces the state to `Closed` and carries its citation out
  to the API, so a consumer can see why.
- API responses would gain a `holiday_status` field (`clear`, `closed`,
  `unknown`) next to `session_state`. The SDK mirrors it. Both default to
  `unknown`.
- The ingestor should keep polling on `Unknown`, since skipping on a guess is
  the failure mode this whole design avoids. It may skip on a sourced
  `Closed`.

## Where closure data could come from

Listed in order of preference, each subject to the rules in CONTRIBUTING:

1. An exchange-published machine-readable calendar or API, if one is free to
   use.
2. A calendar file maintained in this repository, with one entry per closure
   citing the exchange's own circular and its date. Human reviewed, one venue
   per PR, same bundling limit as session verification.
3. A licensed redistributor, if the project holds a licence that allows it.

Scraping exchange websites to build a feed is excluded.

## Open questions

- Format of the in-repo file (TOML per venue-year vs one JSON). It should be
  diffable and reviewable by someone who knows the exchange, not Rust.
- Whether half-day sessions (early closes) need a `Closure` variant. None are
  documented for the verified venues yet.
- Whether to expose the feed through `/v1/sources` so consumers can see which
  venues have holiday coverage.
