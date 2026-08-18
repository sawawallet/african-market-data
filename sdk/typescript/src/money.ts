/**
 * Fixed-point money. No floats anywhere in the price path.
 *
 * Exchange feeds publish scaled integers — MITCH prices are signed int64 with
 * eight implied decimal places — and the whole point of holding that shape end
 * to end is that sums, differences and comparisons stay exact.
 */
import type { CurrencyCode, Price } from "./types.js";

export class MoneyError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MoneyError";
  }
}

/** Build a Price from an integer value already scaled to `scale` decimals. */
export function price(value: bigint, scale: number, currency: CurrencyCode): Price {
  if (!Number.isInteger(scale) || scale < 0 || scale > 18) {
    throw new MoneyError(`scale must be an integer in [0, 18], got ${scale}`);
  }
  return { value, scale, currency };
}

/**
 * Parse a decimal string exactly.
 *
 * Strings, not numbers, because `0.1 + 0.2` is the reason this module exists.
 * Adapters receiving JSON numbers should stringify before calling this — see
 * `fromJsonNumber`.
 */
export function parsePrice(input: string, currency: CurrencyCode, scale = 4): Price {
  const trimmed = input.trim().replace(/,/g, "");
  const m = /^(-)?(\d*)(?:\.(\d*))?$/.exec(trimmed);
  if (!m || (m[2] === "" && (m[3] ?? "") === "")) {
    throw new MoneyError(`not a decimal number: ${JSON.stringify(input)}`);
  }
  const [, sign, whole = "", frac = ""] = m;
  if (frac.length > scale) {
    // Refuse to silently drop precision the source actually provided.
    throw new MoneyError(
      `"${trimmed}" has ${frac.length} decimals but scale is ${scale}; ` +
        `raise the scale or round explicitly`,
    );
  }
  const padded = frac.padEnd(scale, "0");
  const digits = `${whole || "0"}${padded}`;
  const value = BigInt(digits) * (sign === "-" ? -1n : 1n);
  return { value, scale, currency };
}

/**
 * Convert a JSON number to a Price.
 *
 * Uses the number's own shortest round-trip representation, which is the most
 * faithful reading available once a source has already put it through a float.
 * Prefer `parsePrice` on the raw string wherever the source gives you one.
 */
export function fromJsonNumber(n: number, currency: CurrencyCode, scale = 4): Price {
  if (!Number.isFinite(n)) throw new MoneyError(`not a finite number: ${n}`);
  const s = n.toString();
  if (s.includes("e") || s.includes("E")) {
    // Exponential notation: expand via toFixed at the target scale.
    return parsePrice(n.toFixed(scale), currency, scale);
  }
  const frac = s.split(".")[1] ?? "";
  return parsePrice(frac.length > scale ? n.toFixed(scale) : s, currency, scale);
}

/** Rescale to a different number of decimals. Rounds half away from zero. */
export function rescale(p: Price, scale: number): Price {
  if (scale === p.scale) return p;
  if (scale > p.scale) {
    return price(p.value * 10n ** BigInt(scale - p.scale), scale, p.currency);
  }
  const divisor = 10n ** BigInt(p.scale - scale);
  const q = p.value / divisor; // BigInt division truncates toward zero
  const rem = p.value % divisor;
  const absRem = rem < 0n ? -rem : rem;
  // Round half away from zero.
  if (absRem * 2n >= divisor) {
    return price(q + (p.value < 0n ? -1n : 1n), scale, p.currency);
  }
  return price(q, scale, p.currency);
}

function align(a: Price, b: Price): [bigint, bigint, number] {
  assertSameCurrency(a, b);
  const scale = Math.max(a.scale, b.scale);
  return [rescale(a, scale).value, rescale(b, scale).value, scale];
}

export function assertSameCurrency(a: Price, b: Price): void {
  if (a.currency !== b.currency) {
    throw new MoneyError(`currency mismatch: ${a.currency} vs ${b.currency}`);
  }
}

export function add(a: Price, b: Price): Price {
  const [x, y, scale] = align(a, b);
  return price(x + y, scale, a.currency);
}

export function subtract(a: Price, b: Price): Price {
  const [x, y, scale] = align(a, b);
  return price(x - y, scale, a.currency);
}

/** -1 if a < b, 0 if equal, 1 if a > b. */
export function compare(a: Price, b: Price): -1 | 0 | 1 {
  const [x, y] = align(a, b);
  return x < y ? -1 : x > y ? 1 : 0;
}

export function equals(a: Price, b: Price): boolean {
  return compare(a, b) === 0;
}

export function isZero(p: Price): boolean {
  return p.value === 0n;
}

/**
 * Percentage change from `from` to `to`, in basis points.
 *
 * Basis points rather than a float percentage so the result stays exact and
 * comparable. 250 bps is +2.50%.
 */
export function changeBps(from: Price, to: Price): bigint {
  assertSameCurrency(from, to);
  if (isZero(from)) throw new MoneyError("cannot compute change from zero");
  const [x, y] = align(from, to);
  return ((y - x) * 10000n) / x;
}

/** Format for display. This is the only place a Price becomes lossy. */
export function formatPrice(p: Price, opts: { symbol?: boolean } = {}): string {
  const neg = p.value < 0n;
  const abs = neg ? -p.value : p.value;
  const s = abs.toString().padStart(p.scale + 1, "0");
  const whole = s.slice(0, s.length - p.scale) || "0";
  const frac = p.scale > 0 ? `.${s.slice(s.length - p.scale)}` : "";
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  const prefix = opts.symbol === false ? "" : `${p.currency} `;
  return `${neg ? "-" : ""}${prefix}${grouped}${frac}`;
}

/** Convert to a JS number. Lossy by definition — display and charting only. */
export function toNumber(p: Price): number {
  return Number(p.value) / 10 ** p.scale;
}
