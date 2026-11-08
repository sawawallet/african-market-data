# The source ladder: getting data more directly

The README promises "a path from free public sources to direct exchange feeds."
This note is that path, made concrete. It exists because "get the data directly"
is not one decision — it is a different decision per venue, and without a shared
frame every contributor re-litigates it from scratch.

The short version: **directness is a ladder, and the right rung is per venue.**
You climb to the highest rung that is both *reachable* and *worth it* for that
exchange, record which rung you are on, and leave the next rung as a backlog
item. For a venue that only ever publishes a daily PDF, the top of its ladder is
lower than JSE's — and pretending otherwise is how you end up inventing data.

## Two axes, not one

The repo already models one axis and not the other, and conflating them is the
usual mistake.

- **Delay** — how *old* the datum is. Already modelled: `DelayClass`
  (`Realtime` / delayed / `Unknown`) rides on every `Provenance` and drives
  entitlement filtering at the API egress.
- **Directness** — how *close to the exchange* the datum came from. A real-time
  price re-published by an aggregator is low-directness even though it is fresh;
  an official end-of-day file straight from the exchange is high-directness even
  though it is a day old.

They are independent. `kwayisi` is low-directness and `Unknown`-delay. An
exchange's own EOD file is high-directness and end-of-day. A licensed MITCH feed
is high-directness and `Realtime`. **This note is about the directness axis** —
moving quotes up it without ever faking where they came from.

## The ladder

| Rung | Source | Directness | Typical delay | What it costs |
|------|--------|-----------|---------------|---------------|
| 0 | **Third-party aggregator** (kwayisi, secondary listings) | Re-published | EOD / unknown | Free, no SLA, no recourse when it drifts |
| 1 | **The exchange's own published file** — the daily EOD CSV/PDF on its site | From the source | EOD | Free or near-free; a parser, and care with the no-scraping rule |
| 2 | **A direct licensed data product** from the exchange — their EOD or delayed feed, under contract | From the source | EOD to ~15 min | A data agreement + fees, per exchange |
| 3 | **The exchange's native real-time feed** — MITCH / ITCH / FIX over colocation or an extranet | From the source, tick-level | `Realtime` | Data licence + connectivity + conformance work |
| 4 | **A licensed redistributor** (LSEG/Refinitiv, Bloomberg) | One hop from source | `Realtime` | Expensive; convenient but not "direct" |

Two things this table is meant to settle:

**Rungs 1 and 2 are already "direct from source."** They are not real-time, but
they come straight from the exchange with no interpreting middleman. For a
thinly-traded venue that prints a handful of times a day — see the NGX long-tail
note in the README — an official EOD file is not a consolation prize, it is the
correct answer. Do not let "direct" collapse into "rung 3 or nothing."

**Rung 3 is not always reachable, and that is a fact about the venue, not a
failure of effort.** Some exchanges do not offer a third-party real-time
electronic feed at all. Chasing one there is wasted work; rung 1 or 2 is the
ceiling, and the honest move is to record that ceiling.

## Make the rung queryable

Directness should be a property a consumer can read, exactly as delay already
is. Today `/v1/sources` surfaces `capabilities()` and `terms()` per the
`Adapter` contract; a source's rung is implicit in that prose. The follow-up is
to make it explicit — a directness marker alongside `DelayClass` on
`Provenance`, so the API can answer "how direct is this quote?" the same way it
answers "how stale is this quote?" A quote that came off rung 0 and a quote that
came off rung 3 should not be distinguishable only by someone who already knows
the source by name.

Until that lands, this document is the registry of record for which rung each
venue sits on and which rung is next.

## Per-venue worksheet

**Read this table the way you read the coverage table in the README: filled
cells are sourced, blank cells are unknown, and unknown is not a guess.** Only
facts already established in this repository are asserted here. A blank "native
feed" cell does **not** mean a venue has no feed — it means nobody has confirmed
one from a primary source yet, and confirming it is a contribution. Do not fill
a cell from a secondary summary; fill it from the exchange's own documentation,
and cite it here when you do.

| Code | Rung today | Free source today | Native feed (confirmed) | Next rung to research |
|------|-----------|-------------------|-------------------------|-----------------------|
| NGX  | — (modelled only) | — | ITCH / MoldUDP64 via Nasdaq X-Stream; FIX 5.0 spec published¹ | Rung 3: confirm feed access terms, or rung 1 official EOD |
| JSE  | — | — | MITCH (MillenniumIT)¹ | Rung 3 connectivity/licence, or rung 1 official EOD |
| NSX  | — | — | MITCH — runs on the JSE platform¹ | Rung 1 official EOD; rung 3 inherits JSE connectivity |
| GSE  | 0 | kwayisi² | — | Rung 1: does GSE publish an official daily file? |
| NSE  | — | — | MITCH¹ | Rung 1 official EOD; rung 3 licence/connectivity |
| EGX  | — | — | — | Rung 1/2: EGX has a data-dissemination arm — confirm product + terms |
| BRVM | — | — | — | Rung 1: confirm official daily file; kwayisi may serve it² |
| CSE  | — | — | — | Rung 1: confirm official daily file |
| SEM  | — | — | — | Rung 1: confirm official daily file |
| BSE  | — | — | — | Rung 1: confirm official daily file |
| LUSE | — | — | — | Rung 1: confirm official daily file |
| DSE  | — | — | — | Rung 1: confirm official daily file |
| USE  | — | — | — | Rung 1: confirm official daily file |
| ZSE  | — | — | — | Rung 1: confirm official daily file |
| MSE  | — | — | — | Rung 1: confirm official daily file |
| RSE  | — | — | — | Rung 1: confirm official daily file |
| BVMT | — | — | — | Rung 1: confirm official daily file |

¹ Established in the README "Direct exchange feeds" section and implemented as
the `amd-feed` MITCH codec and MoldUDP64 transport. The protocol is known; the
*licence and connectivity* to actually receive it are the open work.

² The kwayisi adapter ships for GSE only (`crates/amd-adapters`). kwayisi/afx
publishes several other African venues under the same free terms; which venues,
and their exact endpoint paths, must be confirmed at `dev.kwayisi.org/apis`
before any are added — one venue per PR, with a captured fixture, per
CONTRIBUTING.

Every blank in columns 2–4 is a research task. The point of the table is to make
the *absence* of a confirmed direct source visible per venue, so the project
works down a real list instead of guessing which exchange to chase next.

## How to climb a rung (the contribution)

For one venue:

1. **Find its real ladder.** Start at the exchange's own site. Does it publish a
   daily results/EOD file a machine can read (rung 1)? Does it advertise a data
   product or a market-data department you would contract with (rung 2)? Does it
   run a known trading platform with a documented feed (rung 3)? Record what you
   find in this table with a citation — including "no official file found,"
   which is itself a finding.
2. **Take the highest reachable rung that is worth it.** For most venues here
   that is rung 1 — an adapter that reads the exchange's own published file,
   tagged honestly: `DelayClass::EndOfDay` (not `Realtime` because it feels
   authoritative), and the directness marker, once it exists, set to "from
   source." Capture a fixture, per CONTRIBUTING.
3. **Respect the no-scraping line.** CONTRIBUTING is explicit: this repo is tied
   to a licensed entity, and scraping an exchange's own site under that
   association is an unforced error. Rung 1 means the exchange's *published data
   file or documented endpoint* — a CSV it posts for download, a documented API
   — not HTML lifted off a page it never meant to be read that way. When in
   doubt, it is a rung-2 conversation (ask the exchange for the file), not a
   scraper.
4. **Leave the next rung as an issue.** If you land rung 1, note what rung 2 or 3
   would take for that venue, so the ladder stays a live backlog.

## What to build to support this

In rough order of value per unit effort:

- **Rung-1 official-file adapters.** One per venue that publishes a parseable
  daily file, decoded straight from the exchange's own file. The single biggest
  jump in directness for the least cost, and no licence required.
- **A directness marker on `Provenance`**, alongside `DelayClass`, so the rung
  is a queryable property surfaced through `/v1/sources` and on every quote —
  not tribal knowledge about which source name means what.
- **A FIX engine adapter** beside the MITCH one. Several platforms (NGX via
  X-Stream among them) expose FIX before, or instead of, a raw binary feed, so
  FIX is often the first reachable rung-3 door.
- **A pcap replay / conformance harness** for `amd-feed`, so the MITCH and
  MoldUDP64 paths are testable against captured real packets before any live
  connectivity exists — the honest way to trust a direct feed you cannot yet
  reach.

## The honest ceiling

Not every venue reaches rung 3, and some do not reach rung 2. A daily PDF may be
the top of a small exchange's ladder, and an adapter that reads that PDF — or
records that no machine-readable file exists at all — is the correct, finished
answer for that venue. The provenance model is what lets the project mix rungs
across seventeen venues and stay truthful about each one, instead of flattening
them into a single false claim of "direct."
