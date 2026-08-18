# Contributing

## The most useful thing you can do

**Verify a venue's trading hours.** Sixteen of the seventeen venues in
`crates/amd-calendar/src/registry.rs` carry `sessions_verified: false`, meaning
nobody has checked those times against the exchange's own published schedule.
They are best-effort and some of them are wrong.

To fix one:

1. Find the venue's own published trading schedule — the exchange's website or
   rulebook, not a third-party summary.
2. Correct the `Window` values if needed.
3. Set `sessions_verified: true` and fill `sessions_source` with a short
   description of where the times came from and when they took effect.
4. Add a test in `crates/amd-calendar/tests/sessions.rs` pinning a time inside
   and outside the window.

NGX shows the shape:

```rust
sessions: NGX_SESSIONS,
sessions_verified: true,
sessions_source: Some("NGX trading window extended to 09:00-16:00 WAT, effective 2026-04-27"),
```

That entry exists because NGX moved its close from 14:00 to 16:00 in April 2026.
Schedules change, so `sessions_source` records *when* the fact was true.

## Adding an adapter

Implement `Adapter` in `crates/amd-adapters/`. Three rules:

**Report your source honestly.** `capabilities()` and `terms()` are surfaced
through `/v1/sources` so consumers can see what they are actually being served.
If the source publishes no timestamp, `timestamps: false` and every quote gets
`Provenance::imputed`. If freshness is not contractually specified, the delay
class is `Unknown` — not `Realtime` because it feels fast.

This is not pedantry. `DelayClass` drives entitlement filtering at the API
boundary. An adapter that overstates its class causes licensed data to be
served to consumers not entitled to it.

**Never parse prices through a float where a string is available.** Use
`Price::parse` on the raw string. `Price::from_f64` exists only for sources that
publish JSON numbers and give you nothing better, and it is documented as such.

**Capture a fixture.** Save a real response under `tests/fixtures/` and write a
test that pins the shape you coded against. When the source changes its schema,
that test tells you which assumption broke.

## Adapters that will not be merged

**Scrapers of exchange websites.** This repository is public and associated with
an entity that holds — or is applying for — market data licences. Publishing
code that scrapes an exchange's own site under that association is an unforced
error, regardless of whether the scraping itself is defensible.

Free, documented APIs are fine. The kwayisi GSE adapter is the reference: no
registration, no key, and the maintainer states it is free and unencumbered.

If you need scraped data, run it in your own deployment. The `Adapter` trait is
public and adapters register at runtime specifically so this is possible without
the code living here.

## Style

Match the surrounding code. Two conventions worth calling out:

- **Comments explain why, not what.** Most comments in this codebase document a
  decision that looks wrong until you know the constraint — why `Price` holds
  `i128`, why the ClickHouse `delay_class` column is an `i8`, why polling
  follows a calendar. Keep that bar.
- **Failures fail closed.** An unrecognised entitlement class denies access. An
  unparseable price is an error, not a zero. A gap in a sequence marks data
  stale rather than publishing a book built on a missing transition.

## Running the tests

```bash
cargo test --workspace                     # offline, no infrastructure
cargo test -p amd-adapters -- --ignored    # hits live third-party APIs
cargo clippy --workspace --all-targets
```

Live tests are `#[ignore]` because CI must never depend on — or hammer — a free
third-party service. Run them deliberately before submitting an adapter change.

If you change SQL, regenerate the offline query cache (see the README) and
commit `.sqlx`.
