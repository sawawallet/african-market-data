import { AdapterRegistry } from "./adapters/registry.js";
import type { Adapter, AdapterContext } from "./adapters/adapter.js";
import { getExchange } from "./exchanges.js";
import { sessionState, stalenessSeconds } from "./calendar.js";
import type { ExchangeCode, Instrument, Quote } from "./types.js";
import { AdapterError } from "./types.js";
import type { SessionState } from "./calendar.js";

export interface ClientOptions {
  adapters?: readonly Adapter[];
  userAgent?: string;
}

/** A quote plus the derived context a caller needs to display it honestly. */
export interface AnnotatedQuote {
  readonly quote: Quote;
  /** Seconds since `provenance.asOf`. */
  readonly stalenessSeconds: number;
  readonly session: SessionState;
  /**
   * True when the venue is open but the datum is older than `staleAfter`.
   * The signal that a price is not moving because nobody is watching it, as
   * opposed to because nobody is trading it.
   */
  readonly stale: boolean;
}

export class MarketDataClient {
  readonly registry: AdapterRegistry;
  readonly #userAgent: string | undefined;

  constructor(opts: ClientOptions = {}) {
    this.registry = new AdapterRegistry(opts.adapters);
    this.#userAgent = opts.userAgent;
  }

  /** Register a licensed or custom adapter. Takes precedence over defaults. */
  use(adapter: Adapter): this {
    this.registry.register(adapter);
    return this;
  }

  exchanges(): ExchangeCode[] {
    return this.registry.supported();
  }

  async instruments(
    exchange: ExchangeCode,
    ctx: AdapterContext = {},
  ): Promise<Instrument[]> {
    return this.#first(exchange).listInstruments(exchange, this.#ctx(ctx));
  }

  async quotes(
    exchange: ExchangeCode,
    symbols?: readonly string[],
    ctx: AdapterContext = {},
  ): Promise<Quote[]> {
    return this.#first(exchange).getQuotes(exchange, symbols, this.#ctx(ctx));
  }

  /**
   * Quotes with freshness and session state attached.
   *
   * `staleAfter` defaults to 15 minutes, which is roughly the point at which a
   * quiet NGX or GSE ticker becomes indistinguishable from a broken feed.
   */
  async annotatedQuotes(
    exchange: ExchangeCode,
    symbols?: readonly string[],
    opts: AdapterContext & { staleAfter?: number; now?: Date } = {},
  ): Promise<AnnotatedQuote[]> {
    const now = opts.now ?? new Date();
    const staleAfter = opts.staleAfter ?? 900;
    const session = sessionState(now, getExchange(exchange));
    const quotes = await this.quotes(exchange, symbols, opts);
    return quotes.map((quote) => {
      const age = stalenessSeconds(quote.provenance.asOf, now);
      return {
        quote,
        stalenessSeconds: age,
        session,
        stale: session === "open" && age > staleAfter,
      };
    });
  }

  #ctx(ctx: AdapterContext): AdapterContext {
    return this.#userAgent ? { ...ctx, userAgent: ctx.userAgent ?? this.#userAgent } : ctx;
  }

  #first(exchange: ExchangeCode): Adapter {
    const [adapter] = this.registry.for(exchange);
    if (!adapter) {
      throw new AdapterError(
        `no adapter registered for ${exchange}. ` +
          `Supported: ${this.registry.supported().join(", ") || "none"}`,
        "client",
      );
    }
    return adapter;
  }
}
