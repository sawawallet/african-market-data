/**
 * Ghana Stock Exchange via the kwayisi GSE API.
 *
 * Chosen as the first adapter because it is the one African source that is
 * unambiguously free to use: no registration, no key, and the maintainer states
 * it is "completely free and unencumbered for everyone". That matters for a
 * public repository — see CONTRIBUTING.md on why this project does not ship
 * scrapers of exchange websites.
 *
 * Two honest limitations, both reflected in the Provenance this adapter emits:
 *
 *  - The API publishes no timestamp, so `asOf` is the receive time and
 *    `asOfImputed` is true. You cannot tell from the payload when a price
 *    printed.
 *  - Freshness is not contractually specified and observed behaviour is close
 *    to end-of-day, so `delay` is "unknown" rather than "realtime".
 *
 * Endpoints: https://dev.kwayisi.org/apis/gse/
 */
import type { Adapter, AdapterContext } from "./adapter.js";
import { AdapterError } from "../types.js";
import type { ExchangeCode, Instrument, Quote } from "../types.js";
import { fromJsonNumber, subtract } from "../money.js";
import { getJson } from "../util/http.js";

const BASE = "https://dev.kwayisi.org/apis/gse";
const CURRENCY = "GHS";
const SCALE = 4;

/** `/live` row. `name` is the ticker; `change` is absolute, not a percentage. */
interface LiveRow {
  name: string;
  price: number;
  change: number;
  volume: number;
}

/** `/equities/{symbol}` detail. Top-level `name` is the ticker. */
interface EquityDetail {
  name: string;
  price: number;
  shares?: number | null;
  capital?: number | null;
  eps?: number | null;
  dps?: number | null;
  company?: {
    name?: string | null;
    sector?: string | null;
    industry?: string | null;
  } | null;
}

function assertGse(exchange: ExchangeCode): void {
  if (exchange !== "GSE") {
    throw new AdapterError(
      `kwayisi serves GSE only, asked for ${exchange}`,
      "kwayisi",
    );
  }
}

function instrument(symbol: string, name?: string, sector?: string): Instrument {
  return {
    exchange: "GSE",
    symbol,
    id: `GSE:${symbol}`,
    currency: CURRENCY,
    ...(name ? { name } : {}),
    ...(sector ? { sector } : {}),
  };
}

export class KwayisiAdapter implements Adapter {
  readonly id = "kwayisi";
  readonly exchanges = ["GSE"] as const satisfies readonly ExchangeCode[];
  readonly terms =
    "Free, no registration. Publishes no timestamp, so freshness is imputed.";

  async listInstruments(
    exchange: ExchangeCode,
    ctx: AdapterContext = {},
  ): Promise<Instrument[]> {
    assertGse(exchange);
    const rows = await this.#get<Array<{ name: string }>>("/equities", ctx);
    return rows.map((r) => instrument(r.name));
  }

  async getQuotes(
    exchange: ExchangeCode,
    symbols?: readonly string[],
    ctx: AdapterContext = {},
  ): Promise<Quote[]> {
    assertGse(exchange);
    const rows = await this.#get<LiveRow[]>("/live", ctx);
    const wanted = symbols ? new Set(symbols.map((s) => s.toUpperCase())) : null;
    const receivedAt = new Date();

    return rows
      .filter((r) => !wanted || wanted.has(r.name.toUpperCase()))
      .map((r) => {
        const last = fromJsonNumber(r.price, CURRENCY, SCALE);
        const change = fromJsonNumber(r.change, CURRENCY, SCALE);
        return {
          instrument: instrument(r.name),
          last,
          // The API gives absolute change, so previous close is exact here.
          previousClose: subtract(last, change),
          volume: BigInt(Math.max(0, Math.trunc(r.volume ?? 0))),
          provenance: {
            source: this.id,
            asOf: receivedAt,
            receivedAt,
            delay: "unknown" as const,
            asOfImputed: true,
          },
        } satisfies Quote;
      });
  }

  /** Company detail for one ticker. Not part of the Adapter interface. */
  async getInstrumentDetail(
    symbol: string,
    ctx: AdapterContext = {},
  ): Promise<Instrument & { sharesOutstanding?: bigint }> {
    const d = await this.#get<EquityDetail>(
      `/equities/${encodeURIComponent(symbol.toUpperCase())}`,
      ctx,
    );
    const base = instrument(
      d.name,
      d.company?.name ?? undefined,
      d.company?.sector ?? undefined,
    );
    const shares = d.shares;
    return shares != null && Number.isFinite(shares)
      ? { ...base, sharesOutstanding: BigInt(Math.trunc(shares)) }
      : base;
  }

  async #get<T>(path: string, ctx: AdapterContext): Promise<T> {
    try {
      return await getJson<T>(`${BASE}${path}`, {
        ...(ctx.signal ? { signal: ctx.signal } : {}),
        ...(ctx.userAgent ? { userAgent: ctx.userAgent } : {}),
      });
    } catch (err) {
      throw new AdapterError(`kwayisi ${path} failed`, this.id, err);
    }
  }
}
