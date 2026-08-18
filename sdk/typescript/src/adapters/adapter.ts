/**
 * The source interface.
 *
 * Adding an exchange means implementing this and registering it. Adapters do
 * not decide policy: they report what their source actually provides, including
 * an honest `delay` and an honest `asOfImputed`. An adapter that claims
 * "realtime" for a source that does not guarantee it is a bug, not an
 * optimism.
 */
import type { Bar, ExchangeCode, Instrument, Quote } from "../types.js";

export interface AdapterContext {
  signal?: AbortSignal;
  userAgent?: string;
}

export interface Adapter {
  /** Stable id recorded in every Provenance this adapter produces. */
  readonly id: string;
  readonly exchanges: readonly ExchangeCode[];
  /** Human-readable licensing note. Shown in the source listing. */
  readonly terms: string;

  /** Every instrument this adapter can serve on the given exchange. */
  listInstruments(exchange: ExchangeCode, ctx?: AdapterContext): Promise<Instrument[]>;

  /** Latest quotes. Omit `symbols` for the whole board. */
  getQuotes(
    exchange: ExchangeCode,
    symbols?: readonly string[],
    ctx?: AdapterContext,
  ): Promise<Quote[]>;

  /** Daily bars, if the source offers history. */
  getHistory?(
    exchange: ExchangeCode,
    symbol: string,
    ctx?: AdapterContext,
  ): Promise<Bar[]>;
}
