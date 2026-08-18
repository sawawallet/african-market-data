/**
 * Print a live GSE board with freshness attached.
 *
 *   npm run example
 */
import { MarketDataClient, changeBps, formatPrice, getExchange } from "../dist/index.js";

const client = new MarketDataClient();
const gse = getExchange("GSE");
const rows = await client.annotatedQuotes("GSE");

const session = rows[0]?.session ?? "closed";
console.log(`\n${gse.name}  ·  ${gse.timezone}  ·  session: ${session}`);
console.log(`${rows.length} instruments\n`);

const movers = rows
  .filter((r) => r.quote.last && r.quote.previousClose)
  .map((r) => ({
    symbol: r.quote.instrument.symbol,
    last: r.quote.last!,
    bps: changeBps(r.quote.previousClose!, r.quote.last!),
    volume: r.quote.volume ?? 0n,
    imputed: r.quote.provenance.asOfImputed,
  }))
  .sort((a, b) => Number(b.bps - a.bps));

const show = [...movers.slice(0, 5), ...movers.slice(-5)];
for (const m of show) {
  const pct = (Number(m.bps) / 100).toFixed(2).padStart(6);
  const vol = m.volume.toLocaleString("en-US").padStart(12);
  console.log(
    `${m.symbol.padEnd(12)} ${formatPrice(m.last).padStart(14)}  ${pct}%  ${vol}`,
  );
}

// The point of the provenance model: say what you do not know.
if (rows[0]?.quote.provenance.asOfImputed) {
  console.log(
    `\n⚠  ${rows[0].quote.provenance.source} publishes no timestamp — ` +
      `"as of" is the receive time, not a print time.`,
  );
}
console.log();
