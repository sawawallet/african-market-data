# African Market Data

**Open market data infrastructure for African stock exchanges.** One normalised
schema across NGX, JSE, GSE, NSE Kenya, EGX, BRVM and twelve more venues — with
fixed-point prices, honest provenance on every quote, and a path from free
public sources to direct exchange feeds.

[![CI](https://github.com/sawawallet/african-market-data/actions/workflows/ci.yml/badge.svg)](https://github.com/sawawallet/african-market-data/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## Why this exists

If you have tried to build anything on African market data, you already know
the problem: every venue publishes differently, half of them publish nothing a
machine can read, symbols do not reconcile across sources, and the vendors that
do aggregate charge for a 30-minute refresh and call it live.

This project is the layer underneath that. It normalises what is available,
tells you honestly how stale it is, and is architected so that swapping a free
polled source for a licensed direct exchange feed is a configuration change
rather than a rewrite.

## What makes it different

**Prices are fixed-point integers, never floats.** MITCH — the protocol JSE,
NSX and NSE Kenya speak — publishes prices as signed `i64` with eight implied
decimal places. That shape is preserved end to end, through the bus, into
ClickHouse, and out of the API. A float creeps in once and reconciliation breaks
silently months later.

```
GHS 7.03  →  { minor: 703000000, scale: 8, currency: "GHS" }
```

**Every quote carries its provenance.** Source, exchange timestamp, receive
time, delay class, and whether the timestamp was imputed because the source
publishes none.

```jsonc
{
  "instrument": "GSE:MTNGH",
  "last": { "minor": 703000000, "scale": 8, "currency": "GHS" },
  "provenance": {
    "source": "kwayisi",
    "as_of": "2026-08-18T02:20:08Z",
    "delay": "unknown",
    "as_of_imputed": true   // this source publishes no timestamp — we say so
  },
  "staleness_seconds": 0,
  "stale": false
}
```

This matters more here than on liquid markets. NGX clears roughly 42,000 deals
across a seven-hour session spread over ~150 tickers, so the long tail prints a
handful of times a day. **A stale price and a flat price look identical**, and
without `as_of` a consumer reads a two-hour-old quote as "the market is quiet
today".

**Entitlements are enforced against the datum, not the route.** Real-time
exchange data is licensed per delay class. The class travels on the quote, and
the check runs at the egress boundary — so a free tier cannot be served
licensed real-time data because someone wired a route wrong.

## Quick start

```bash
git clone https://github.com/sawawallet/african-market-data
cd african-market-data
cargo run -p amd-api
```

That is the whole first run. No database, no message broker, no API key — the
API boots on adapters alone and serves live Ghana Stock Exchange data:

```bash
curl 'localhost:8080/v1/exchanges/GSE/quotes?symbols=MTNGH'
curl 'localhost:8080/v1/exchanges'      # all 17 venues, sessions, current state
curl 'localhost:8080/v1/sources'        # what each source provides, and its terms
```

Infrastructure is additive. Bring up the full stack when you want persistence
and streaming:

```bash
cp .env.example .env
docker compose -f deploy/docker-compose.yml up -d
cargo run -p amd-ingestor    # polls on each venue's calendar → NATS → ClickHouse
```

`/v1/exchanges/{code}/stream` (Server-Sent Events) becomes available once NATS
is up. Without it the endpoint returns 503 with a hint rather than failing
obscurely.

## Coverage

Seventeen venues are modelled. `verified` means the session times were checked
against the exchange's own published schedule — **the rest are best effort**,
and correcting one is the single most useful contribution this project takes.

| Code | Exchange | Country | Currency | Session (local) | Verified | Data source |
|------|----------|---------|----------|-----------------|----------|-------------|
| NGX  | Nigerian Exchange | NG | NGN | 09:00–16:00 | ✅ | — |
| JSE  | Johannesburg Stock Exchange | ZA | ZAR | 09:00–17:00 | — | — |
| NSX  | Namibian Stock Exchange | NA | NAD | 09:00–17:00 | — | — |
| GSE  | Ghana Stock Exchange | GH | GHS | 10:00–15:00 | ✅ | kwayisi |
| NSE  | Nairobi Securities Exchange | KE | KES | 09:31–15:00 | ✅ | — |
| EGX  | Egyptian Exchange | EG | EGP | 10:00–14:30 (Sun–Thu) | — | — |
| BRVM | Bourse Régionale des Valeurs Mobilières | 8 UEMOA states | XOF | 09:45–14:00 | ✅ | — |
| CSE  | Casablanca Stock Exchange | MA | MAD | 09:30–15:20 | — | — |
| SEM  | Stock Exchange of Mauritius | MU | MUR | 09:00–13:30 | — | — |
| BSE  | Botswana Stock Exchange | BW | BWP | 10:25–11:55, 12:05–13:20 | ✅ | — |
| LUSE | Lusaka Securities Exchange | ZM | ZMW | 10:00–14:00 | — | — |
| DSE  | Dar es Salaam Stock Exchange | TZ | TZS | 09:31–16:00 | ✅ | — |
| USE  | Uganda Securities Exchange | UG | UGX | 09:30–12:00 | — | — |
| ZSE  | Zimbabwe Stock Exchange | ZW | USD | 09:00–15:30 | — | — |
| MSE  | Malawi Stock Exchange | MW | MWK | 09:00–14:00 | — | — |
| RSE  | Rwanda Stock Exchange | RW | RWF | 09:00–12:00 | — | — |
| BVMT | Bourse de Tunis | TN | TND | 09:00–14:10 | — | — |

A venue with two windows lists both: Botswana trades either side of a
ten-minute intra-day auction, and `sessions` is an array precisely so that gap
is representable rather than flattened into one long window.

**Public holidays are not modelled.** A wrong holiday calendar is worse than an
absent one, because it silently reports a closed market as open. `session_state`
claims only "this is a scheduled trading weekday and the clock is inside a
window", which is exactly what it can prove.

### Sources

Only sources that are **unambiguously free to use** ship in this repository. It
does not scrape exchange websites.

| Source | Venues | Freshness | Terms |
|--------|--------|-----------|-------|
| [kwayisi](https://dev.kwayisi.org/apis/gse/) | GSE | No timestamp published; observed near end-of-day | Free, no registration |

Licensed feeds — NGX direct via Nasdaq X-Stream, JSE MITCH, commercial
aggregators — are implemented separately and registered at runtime by whoever
holds the licence. See [Architecture](#architecture).

## Architecture

```
crates/
  amd-core        Types. Fixed-point money, instruments, quotes, provenance.
  amd-calendar    Sessions, exchange-local dates, staleness. IANA tzdb via jiff.
  amd-adapters    Source adapters + the Adapter trait. Free sources only.
  amd-bus         NATS JetStream. Subject taxonomy shaped for leaf nodes.
  amd-store       ClickHouse tick archive + Postgres reference/entitlements.
  amd-ingestor    Polls on each venue's calendar, publishes only what moved.
  amd-api         Axum. REST snapshots, SSE streaming, entitlement filtering.
  amd-feed        MITCH codec, gap recovery, book builder (JSE, NSX, NSE).
sdk/typescript    npm client (in progress)
deploy/           compose stack, ClickHouse schema, Postgres migrations
```

Two design choices carry most of the weight:

**NATS JetStream with leaf nodes**, because venue feed handlers must run
colocated — Lagos for NGX, Johannesburg for JSE and NSX — and a leaf node
forwards upstream while buffering through a WAN partition. That is what makes a
Lagos-to-central link survivable without reconnect logic in the handler. Subjects
put the venue high in the hierarchy (`amd.v1.NGX.quote.MTNN`) precisely so a
leaf can export `amd.v1.NGX.>` and a partition in one region cannot affect
another.

**SSE rather than WebSocket** for streaming. Price updates are one-way. SSE
auto-reconnects, survives corporate proxies, and needs no heartbeat protocol.
WebSocket only earns its complexity once there is order entry to carry back.

### Direct exchange feeds

The architecture anticipates replacing polled sources with direct connectivity.
Two protocol families cover the major venues:

- **MITCH** (MillenniumIT) — JSE, NSX, NSE Kenya. UDP multicast A/B feeds with
  TCP replay and recovery channels. The LSE publishes the same protocol openly
  as **MIT303**, so `amd-feed` implements the codec today: unit header, the
  book-building message set, implied sequencing, and the tiered gap-recovery
  state machine — all tested offline against synthetic packets.
- **ITCH / MoldUDP64** (Nasdaq X-Stream) — NGX, which runs X-Stream with the
  X-Gen market database and publishes a FIX 5.0 specification.

`amd-core`'s `DEFAULT_SCALE` is 8 for exactly this reason: MITCH prices land
without rescaling, and therefore without rounding at ingestion.

Recovery is where feed handlers actually go wrong, so it was built before any
production data path. Four tiers, with **per-instrument quarantine** so one gap
degrades one symbol rather than the venue:

| Tier | Trigger | Response |
|------|---------|----------|
| 0 | Single-path loss | A/B arbitration — take whichever line arrives first |
| 1 | Small gap | Replay channel, rolling 65,000-message window |
| 2 | Gap past that window, or replay budget spent | Snapshot recovery |
| 3 | Sequence resets to 1 | Exchange failover — *not* a catastrophic gap |

Two traps are handled explicitly. **A reset to 1 is a failover**, and a naive
detector reads it as an enormous backwards jump and fires full recovery on every
instrument at once — precisely when the exchange is already degraded. And
**replay quotas are per CompID per day**, so a reconnect loop can burn the day's
allowance in minutes and leave no recovery path; the handler governs itself
rather than trusting the server to.

Distinguishing a restart from an ordinary A/B duplicate is genuinely ambiguous
from the sequence alone — both look like "a number below what we expect". The
separating invariant is that *a single line never goes backwards except on
restart*, so high-water marks are tracked per line rather than globally.

**Book building.** MITCH is market-by-order, and most of its messages carry no
instrument identifier at all — `Order Modified` and `Order Executed` name only
an order reference. So `amd-feed` maintains an order pool alongside per-venue
books, and levels keep their queue of order ids rather than just an aggregate
size. Discarding queue position would still yield correct top-of-book quotes,
but it is exactly the information a market-by-order feed carries and a quote API
does not — throwing it away would forfeit the reason for taking the feed.

Priority is handled where it is easy to get wrong: a resize at an unchanged
price keeps its place, a price move always joins the back of the new level, and
the exchange can revoke priority outright even at the same price. Partial fills
reduce size without costing the remainder its position.

Inconsistencies are counted rather than swallowed — unknown orders, desynced
levels, oversized executions, duplicate ids. A handler that halts on the first
one is useless; one that hides them is worse. A rising `unknown_order` count is
the signature of a gap that recovery missed.

The book projects into the same `Quote` the polled adapters emit, which is what
makes a licensed direct feed substitutable for a free source without anything
downstream noticing.

## Development

```bash
cargo test --workspace                        # 118 tests, no network
cargo test -p amd-adapters -- --ignored       # hits the live kwayisi API
cargo clippy --workspace --all-targets
```

The build is offline by default — `.sqlx` holds the prepared query cache, so
you do not need Postgres to compile. Regenerate after changing SQL:

```bash
docker compose -f deploy/docker-compose.yml up -d postgres
export DATABASE_URL=postgres://amd:amd_dev@localhost:55432/amd
sqlx migrate run --source deploy/postgres/migrations
cargo sqlx prepare --workspace -- --all-targets
```

> Host ports for Postgres (55432) and Redis (56379) are offset because a native
> install commonly owns 5432/6379 on a dev machine and wins the loopback race
> against a container bound to `0.0.0.0`.

To exercise the pipeline outside market hours — every African venue is closed
for most of a working day elsewhere — set `AMD_FORCE_POLL=1` on the ingestor.
Never in production.

## Contributing

**[Eleven venues need their trading hours checked][venues]** — one issue each,
no Rust beyond editing a struct literal. If you know one of these exchanges, you
are better placed to fix it than anyone reading its rulebook cold.

[venues]: https://github.com/sawawallet/african-market-data/issues?q=is%3Aissue+is%3Aopen+label%3Avenue-verification

The most valuable contributions, in order:

1. **Verify a venue's session times** against its own published schedule and
   flip `sessions_verified` in `crates/amd-calendar/src/registry.rs`. Eleven
   venues still need this.

   Both venues verified so far were wrong the same way: the window started at
   the pre-open or auction-call time rather than at continuous trading, because
   that is the figure secondary listings quote as "trading hours". `sessions`
   means the window in which trades execute continuously — an auction call is
   `PreOpen`. Read the venue's rulebook for its phase names.
2. **Add an adapter** for a venue that publishes free, documented data.
3. **Holiday calendars**, once there is a source worth trusting.

See [CONTRIBUTING.md](CONTRIBUTING.md). Adapters that scrape exchange websites
will not be merged — see the reasoning there.

## License

MIT. See [LICENSE](LICENSE).

Market data served through this software remains subject to the terms of its
source. This project grants no entitlement to any exchange's data.
