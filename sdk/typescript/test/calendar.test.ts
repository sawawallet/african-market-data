import { test } from "node:test";
import assert from "node:assert/strict";
import { EXCHANGES, unverifiedExchanges } from "../src/exchanges.ts";
import { isTradingDay, sessionDate, sessionState, stalenessSeconds } from "../src/calendar.ts";

const NGX = EXCHANGES.NGX;
const EGX = EXCHANGES.EGX;

test("NGX session spans the extended 09:00-16:00 WAT window", () => {
  // Tuesday 2026-08-18. WAT is UTC+1 with no DST.
  assert.equal(sessionState(new Date("2026-08-18T08:30:00Z"), NGX), "open");  // 09:30
  assert.equal(sessionState(new Date("2026-08-18T14:30:00Z"), NGX), "open");  // 15:30
  assert.equal(sessionState(new Date("2026-08-18T07:30:00Z"), NGX), "pre-open"); // 08:30
  assert.equal(sessionState(new Date("2026-08-18T15:30:00Z"), NGX), "closed"); // 16:30
});

test("15:30 WAT would have been closed under the old 14:00 close", () => {
  // Guards the 2026-04-27 extension against a silent regression.
  assert.equal(sessionState(new Date("2026-08-18T14:30:00Z"), NGX), "open");
});

test("weekends are closed", () => {
  const saturday = new Date("2026-08-22T11:00:00Z");
  assert.equal(isTradingDay(saturday, NGX), false);
  assert.equal(sessionState(saturday, NGX), "closed");
});

test("Egypt trades Sunday to Thursday", () => {
  assert.equal(isTradingDay(new Date("2026-08-23T09:00:00Z"), EGX), true);  // Sunday
  assert.equal(isTradingDay(new Date("2026-08-21T09:00:00Z"), EGX), false); // Friday
});

test("session date uses exchange-local time, not UTC", () => {
  // 23:30 UTC is already the next day in Nairobi (UTC+3).
  assert.equal(sessionDate(new Date("2026-08-18T23:30:00Z"), EXCHANGES.NSE), "2026-08-19");
  assert.equal(sessionDate(new Date("2026-08-18T23:30:00Z"), NGX), "2026-08-19");
  assert.equal(sessionDate(new Date("2026-08-18T12:00:00Z"), NGX), "2026-08-18");
});

test("staleness never goes negative on clock skew", () => {
  const now = new Date("2026-08-18T12:00:00Z");
  assert.equal(stalenessSeconds(new Date("2026-08-18T11:45:00Z"), now), 900);
  assert.equal(stalenessSeconds(new Date("2026-08-18T12:05:00Z"), now), 0);
});

test("registry is honest about what has not been verified", () => {
  const unverified = unverifiedExchanges();
  // NGX is the one we checked against a primary source.
  assert.ok(!unverified.some((e) => e.code === "NGX"));
  assert.ok(unverified.length > 0, "unverified list should not be silently empty");
});
