/**
 * Core domain types.
 *
 * Two rules govern everything here:
 *
 *  1. Prices are fixed-point integers, never floats. Exchange feeds publish
 *     scaled integers (MITCH uses int64 with eight implied decimals) and the
 *     moment a price becomes a float, reconciliation breaks silently months
 *     later. See ./money.ts.
 *
 *  2. Every quote carries its provenance. On African venues a stale price and
 *     a flat price look identical, so a consumer that cannot see `asOf` and
 *     `delay` will read a two-hour-old print as "the market is quiet today".
 */

/** ISO 4217 alphabetic code, e.g. "NGN", "ZAR", "GHS". */
export type CurrencyCode = string;

/** Exchange MIC-ish short code used throughout this library, e.g. "NGX". */
export type ExchangeCode =
  | "NGX" | "JSE" | "NSX" | "GSE" | "NSE" | "EGX" | "BRVM" | "CSE"
  | "SEM" | "BSE" | "LUSE" | "DSE" | "USE" | "ZSE" | "MSE" | "RSE" | "BVMT";

/**
 * A price as an integer plus a decimal scale.
 *
 * `{ value: 3542n, scale: 2 }` is 35.42. Arithmetic stays exact; conversion to
 * a JS number happens only at the display boundary, deliberately.
 */
export interface Price {
  readonly value: bigint;
  /** Number of implied decimal places. */
  readonly scale: number;
  readonly currency: CurrencyCode;
}

/**
 * How far behind the market a datum is, as licensed rather than as measured.
 *
 * This travels with the data from ingestion so an API layer can enforce
 * entitlements per consumer tier, instead of discovering at audit time that a
 * free consumer was served licensed real-time data.
 */
export type DelayClass =
  /** Direct from the exchange feed, no contractual delay. */
  | "realtime"
  /** Contractually delayed, typically 15 or 30 minutes. */
  | "delayed"
  /** Official end-of-day close. */
  | "eod"
  /** Source makes no claim. Treat as untrusted for anything time-sensitive. */
  | "unknown";

/** Where a datum came from and how much to trust its freshness. */
export interface Provenance {
  /** Adapter id, e.g. "kwayisi". */
  readonly source: string;
  /** Exchange timestamp if the source publishes one, else the source's own. */
  readonly asOf: Date;
  /** When this process received it. `asOf` minus this is your feed lag. */
  readonly receivedAt: Date;
  readonly delay: DelayClass;
  /**
   * True when the source publishes no usable timestamp and `asOf` is the
   * receive time standing in for it. Never chart these as if they were prints.
   */
  readonly asOfImputed: boolean;
}

/** A tradeable instrument on one venue. */
export interface Instrument {
  readonly exchange: ExchangeCode;
  /** Ticker as the venue publishes it, e.g. "MTNN", "NGXGROUP". */
  readonly symbol: string;
  /** Stable internal id: `${exchange}:${symbol}`. */
  readonly id: string;
  readonly name?: string;
  readonly currency: CurrencyCode;
  readonly isin?: string;
  readonly sector?: string;
}

/** A point-in-time price observation. */
export interface Quote {
  readonly instrument: Instrument;
  readonly last?: Price;
  readonly previousClose?: Price;
  readonly open?: Price;
  readonly high?: Price;
  readonly low?: Price;
  readonly bid?: Price;
  readonly ask?: Price;
  /** Shares traded in the session. */
  readonly volume?: bigint;
  /** Number of trades in the session, where the venue publishes it. */
  readonly trades?: number;
  readonly provenance: Provenance;
}

/** One session's summary bar. */
export interface Bar {
  readonly instrument: Instrument;
  /** Session date in the exchange's local timezone, as YYYY-MM-DD. */
  readonly date: string;
  readonly open?: Price;
  readonly high?: Price;
  readonly low?: Price;
  readonly close: Price;
  readonly volume?: bigint;
  readonly provenance: Provenance;
}

/** A continuous trading window in exchange-local wall-clock time. */
export interface TradingWindow {
  /** "HH:MM", exchange-local. */
  readonly open: string;
  /** "HH:MM", exchange-local. */
  readonly close: string;
}

export interface Exchange {
  readonly code: ExchangeCode;
  readonly name: string;
  /** ISO 3166-1 alpha-2, or a list where the venue is regional (BRVM). */
  readonly countries: readonly string[];
  readonly currency: CurrencyCode;
  /** IANA timezone, e.g. "Africa/Lagos". */
  readonly timezone: string;
  /** Continuous session windows. Auctions outside these are not modelled yet. */
  readonly sessions: readonly TradingWindow[];
  /** ISO weekday numbers that are trading days. 1 = Monday. */
  readonly tradingDays: readonly number[];
  /**
   * Whether the session times above have been checked against the venue's own
   * published schedule. Unverified entries are best-effort and should not be
   * relied on for anything that matters — see CONTRIBUTING.
   */
  readonly sessionsVerified: boolean;
  /** Where the session times came from, so the next person can re-check. */
  readonly sessionsSource?: string;
}

/** Thrown when an adapter cannot serve a request. */
export class AdapterError extends Error {
  constructor(
    override readonly message: string,
    readonly source: string,
    override readonly cause?: unknown,
  ) {
    super(message);
    this.name = "AdapterError";
  }
}
