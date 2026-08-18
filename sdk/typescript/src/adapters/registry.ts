import type { Adapter } from "./adapter.js";
import type { ExchangeCode } from "../types.js";
import { KwayisiAdapter } from "./kwayisi.js";

/**
 * Adapters shipped by default.
 *
 * Only sources that are unambiguously free to use in a public project belong
 * here. Licensed feeds (NGX direct, JSE MITCH, commercial aggregators) are
 * configured by the consumer and registered at runtime, so this package never
 * implies an entitlement nobody holds.
 */
export function defaultAdapters(): Adapter[] {
  return [new KwayisiAdapter()];
}

export class AdapterRegistry {
  readonly #byExchange = new Map<ExchangeCode, Adapter[]>();

  constructor(adapters: readonly Adapter[] = defaultAdapters()) {
    for (const a of adapters) this.register(a);
  }

  register(adapter: Adapter): this {
    for (const ex of adapter.exchanges) {
      const list = this.#byExchange.get(ex) ?? [];
      // Later registrations take precedence, so a consumer's licensed adapter
      // overrides a free one for the same venue without extra ceremony.
      this.#byExchange.set(ex, [adapter, ...list.filter((a) => a.id !== adapter.id)]);
    }
    return this;
  }

  /** Adapters serving an exchange, preferred first. */
  for(exchange: ExchangeCode): Adapter[] {
    return this.#byExchange.get(exchange) ?? [];
  }

  supported(): ExchangeCode[] {
    return [...this.#byExchange.keys()];
  }

  all(): Adapter[] {
    return [...new Set([...this.#byExchange.values()].flat())];
  }
}
