import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { EXCHANGES } from "../src/exchanges.ts";

// The Rust registry is the source of truth. This test parses it and fails if
// the SDK table drifts, which had happened: nine venues were verified in Rust
// while the SDK still shipped the original unchecked windows.
const RUST = readFileSync(
  new URL("../../../crates/amd-calendar/src/registry.rs", import.meta.url),
  "utf8",
);

const hhmm = (h: string, m: string) => `${h.padStart(2, "0")}:${m.padStart(2, "0")}`;

function windows(listSrc: string, consts: Map<string, string[]>): string[] {
  const named = consts.get(listSrc.trim());
  if (named) return named;
  throw new Error(`unresolved sessions: ${listSrc}`);
}

const consts = new Map<string, string[]>();
for (const m of RUST.matchAll(/const (\w+_SESSIONS): &\[Window\] = &\[(.*?)\];/g)) {
  const ws = [...m[2]!.matchAll(/Window::new\((\d+), (\d+), (\d+), (\d+)\)/g)].map(
    (w) => `${hhmm(w[1]!, w[2]!)}-${hhmm(w[3]!, w[4]!)}`,
  );
  consts.set(m[1]!, ws);
}

const DAYS: Record<string, number> = { MON: 1, TUE: 2, WED: 3, THU: 4, FRI: 5, SAT: 6, SUN: 0 };
const expandDays = (name: string): number[] =>
  name === "MON_FRI" ? [1, 2, 3, 4, 5] : name === "SUN_THU" ? [0, 1, 2, 3, 4]
  : name.split("|").map((d) => DAYS[d.trim()]!);

interface RustVenue {
  code: string; currency: string; timezone: string; sessions: string[];
  days: number[]; verified: boolean;
}

const venues: RustVenue[] = [];
const vstart = RUST.indexOf("pub static VENUES");
for (const m of RUST.slice(vstart).matchAll(/Venue \{\s+code: ExchangeCode::(\w+),(.*?)\n    \},/gs)) {
  const body = m[2]!;
  const cur = /Currency::(?:new\(\*b")?([A-Z]{3})/.exec(body)![1]!;
  venues.push({
    code: m[1]!.toUpperCase(),
    currency: cur,
    timezone: /timezone: "([^"]+)"/.exec(body)![1]!,
    sessions: windows(/sessions: (\w+),/.exec(body)![1]!, consts),
    days: expandDays(/trading_days: ([\w| ]+),/.exec(body)![1]!),
    verified: /sessions_verified: true/.test(body),
  });
}

test("parser found every venue", () => {
  assert.equal(venues.length, Object.keys(EXCHANGES).length);
});

for (const v of venues) {
  test(`SDK matches Rust registry for ${v.code}`, () => {
    const ex = EXCHANGES[v.code as keyof typeof EXCHANGES];
    assert.ok(ex, `${v.code} missing from SDK`);
    assert.equal(ex.currency, v.currency);
    assert.equal(ex.timezone, v.timezone);
    assert.deepEqual(ex.sessions.map((w) => `${w.open}-${w.close}`), v.sessions);
    assert.deepEqual([...ex.tradingDays], v.days);
    assert.equal(ex.sessionsVerified, v.verified);
    if (v.verified) assert.ok(ex.sessionsSource, "verified venue needs a source");
  });
}
