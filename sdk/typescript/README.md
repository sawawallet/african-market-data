# african-market-data (TypeScript SDK)

Client for the [African Market Data](../../README.md) API, plus the core value
types so a browser or Node consumer handles prices and provenance the same way
the Rust services do.

**Status: in progress.** The core types, fixed-point money and exchange
calendar are implemented and tested; the HTTP client against `amd-api` is not
written yet. The Rust workspace is the reference implementation.

```bash
npm install && npm test    # 16 tests
npm run example            # builds, then prints a live GSE board
```

Prices are fixed-point here for the same reason they are in Rust — see
`src/money.ts`. `parsePrice` takes a string and refuses to silently drop
precision; `toNumber` is the only lossy conversion and exists for charting.
