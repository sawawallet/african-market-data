/**
 * Session state per exchange.
 *
 * Timezone handling uses Intl rather than a date library so the package stays
 * dependency-free. Public holidays are deliberately not modelled yet: a wrong
 * holiday calendar is worse than an absent one, because it silently reports a
 * closed market as open. See `isTradingDay` for what this does and does not
 * claim.
 */
import type { Exchange } from "./types.js";

export type SessionState = "pre-open" | "open" | "closed";

interface LocalParts {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  weekday: number; // 0 = Sunday
}

const WEEKDAYS: Record<string, number> = {
  Sun: 0, Mon: 1, Tue: 2, Wed: 3, Thu: 4, Fri: 5, Sat: 6,
};

/** Break an instant into wall-clock parts in the exchange's timezone. */
export function localParts(at: Date, timezone: string): LocalParts {
  const fmt = new Intl.DateTimeFormat("en-US", {
    timeZone: timezone,
    year: "numeric", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", weekday: "short",
    hour12: false,
  });
  const parts: Record<string, string> = {};
  for (const p of fmt.formatToParts(at)) {
    if (p.type !== "literal") parts[p.type] = p.value;
  }
  // Intl renders midnight as "24" in some ICU versions under hour12:false.
  const hour = Number(parts.hour) % 24;
  return {
    year: Number(parts.year),
    month: Number(parts.month),
    day: Number(parts.day),
    hour,
    minute: Number(parts.minute),
    weekday: WEEKDAYS[parts.weekday ?? "Sun"] ?? 0,
  };
}

/** Session date in the exchange's timezone, as YYYY-MM-DD. */
export function sessionDate(at: Date, exchange: Exchange): string {
  const p = localParts(at, exchange.timezone);
  const mm = String(p.month).padStart(2, "0");
  const dd = String(p.day).padStart(2, "0");
  return `${p.year}-${mm}-${dd}`;
}

function toMinutes(hhmm: string): number {
  const [h = "0", m = "0"] = hhmm.split(":");
  return Number(h) * 60 + Number(m);
}

/**
 * Whether `at` falls on a scheduled trading weekday for this venue.
 *
 * This checks the weekday only. Public holidays are NOT modelled, so a true
 * result means "not a weekend for this venue", not "the market is definitely
 * trading". Do not use it as the sole gate for anything that costs money.
 */
export function isTradingDay(at: Date, exchange: Exchange): boolean {
  const { weekday } = localParts(at, exchange.timezone);
  return exchange.tradingDays.includes(weekday);
}

/** Current session state, weekday-accurate and holiday-blind. */
export function sessionState(at: Date, exchange: Exchange): SessionState {
  if (!isTradingDay(at, exchange)) return "closed";
  const p = localParts(at, exchange.timezone);
  const now = p.hour * 60 + p.minute;
  let earliestOpen = Infinity;
  for (const w of exchange.sessions) {
    const open = toMinutes(w.open);
    const close = toMinutes(w.close);
    if (now >= open && now < close) return "open";
    earliestOpen = Math.min(earliestOpen, open);
  }
  return now < earliestOpen ? "pre-open" : "closed";
}

export function isOpen(at: Date, exchange: Exchange): boolean {
  return sessionState(at, exchange) === "open";
}

/**
 * How stale a quote is, in seconds.
 *
 * Use this rather than eyeballing `asOf`. On thin African venues a price that
 * has not moved in two hours and a price that is two hours stale look
 * identical in a UI, and only this number tells them apart.
 */
export function stalenessSeconds(asOf: Date, now: Date = new Date()): number {
  return Math.max(0, Math.round((now.getTime() - asOf.getTime()) / 1000));
}
