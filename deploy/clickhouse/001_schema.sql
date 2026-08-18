-- Tick archive.
--
-- Prices are stored as the scaled integer plus its scale, never as a float or
-- a Decimal with a guessed precision. Decimal64(8) would work for every venue
-- we currently target, but MITCH already hands us an i64 at scale 8 and the
-- point of this schema is that the number reaching disk is byte-for-byte the
-- number the exchange published.
--
-- The archive is not a byproduct. A complete, gap-annotated tick history of
-- African venues is an asset nobody else holds, so this table is append-only
-- and never updated in place.

CREATE DATABASE IF NOT EXISTS amd;

CREATE TABLE IF NOT EXISTS amd.quotes
(
    exchange        LowCardinality(String),
    symbol          LowCardinality(String),
    -- Exchange timestamp, or the receive time where the source publishes none.
    as_of           DateTime64(9, 'UTC'),
    received_at     DateTime64(9, 'UTC'),
    source          LowCardinality(String),
    delay_class     Enum8('realtime' = 1, 'delayed' = 2, 'eod' = 3, 'unknown' = 4),
    as_of_imputed   UInt8,
    -- Feed sequence number where the protocol provides one. MITCH and ITCH do;
    -- REST sources do not, and a NULL here is meaningful rather than missing.
    sequence        Nullable(UInt64),
    -- Reconstructed through a recovery path rather than received live. MITCH
    -- snapshot recovery does not preserve original timestamps, so anything
    -- flagged here must be excluded from microstructure analytics.
    recovered       UInt8,

    currency        LowCardinality(String),
    price_scale     UInt8,
    last            Nullable(Int128),
    previous_close  Nullable(Int128),
    open            Nullable(Int128),
    high            Nullable(Int128),
    low             Nullable(Int128),
    bid             Nullable(Int128),
    ask             Nullable(Int128),
    volume          Nullable(UInt64),
    trades          Nullable(UInt32),

    -- Ingestion time, for reconciling a backfill against a live write.
    inserted_at     DateTime64(3, 'UTC') DEFAULT now64(3)
)
ENGINE = MergeTree
PARTITION BY (exchange, toYYYYMM(as_of))
ORDER BY (exchange, symbol, as_of)
-- Same venue, symbol, source and instant is the same observation. Deduplicating
-- on replay matters because JetStream redelivery is at-least-once and a
-- restarted archiver will re-consume.
PRIMARY KEY (exchange, symbol, as_of)
SETTINGS index_granularity = 8192;

CREATE TABLE IF NOT EXISTS amd.bars
(
    exchange        LowCardinality(String),
    symbol          LowCardinality(String),
    -- Session date in the venue's own timezone, which is not always the UTC
    -- date of any contained timestamp.
    session_date    Date,
    source          LowCardinality(String),
    delay_class     Enum8('realtime' = 1, 'delayed' = 2, 'eod' = 3, 'unknown' = 4),
    currency        LowCardinality(String),
    price_scale     UInt8,
    open            Nullable(Int128),
    high            Nullable(Int128),
    low             Nullable(Int128),
    close           Int128,
    volume          Nullable(UInt64),
    as_of           DateTime64(9, 'UTC'),
    inserted_at     DateTime64(3, 'UTC') DEFAULT now64(3)
)
-- Replacing rather than plain MergeTree: an official close supersedes an
-- intraday snapshot for the same session, and corrections do get published.
ENGINE = ReplacingMergeTree(inserted_at)
PARTITION BY (exchange, toYear(session_date))
ORDER BY (exchange, symbol, session_date);

-- Daily rollup maintained on write, so a chart query never scans raw ticks.
CREATE TABLE IF NOT EXISTS amd.ohlcv_daily
(
    exchange        LowCardinality(String),
    symbol          LowCardinality(String),
    session_date    Date,
    currency        LowCardinality(String),
    price_scale     UInt8,
    open            AggregateFunction(argMin, Nullable(Int128), DateTime64(9, 'UTC')),
    high            AggregateFunction(max, Nullable(Int128)),
    low             AggregateFunction(min, Nullable(Int128)),
    close           AggregateFunction(argMax, Nullable(Int128), DateTime64(9, 'UTC')),
    volume          AggregateFunction(max, Nullable(UInt64)),
    tick_count      AggregateFunction(count)
)
ENGINE = AggregatingMergeTree
PARTITION BY (exchange, toYear(session_date))
ORDER BY (exchange, symbol, session_date);

CREATE MATERIALIZED VIEW IF NOT EXISTS amd.ohlcv_daily_mv
TO amd.ohlcv_daily
AS SELECT
    exchange,
    symbol,
    -- Session date is computed at write time from the exchange timezone and
    -- passed in; this view keys on the as_of date as a fallback only.
    toDate(as_of)                    AS session_date,
    currency,
    any(price_scale)                 AS price_scale,
    argMinState(last, as_of)         AS open,
    maxState(last)                   AS high,
    minState(last)                   AS low,
    argMaxState(last, as_of)         AS close,
    maxState(volume)                 AS volume,
    countState()                     AS tick_count
FROM amd.quotes
-- Recovered data has synthetic timestamps and would corrupt the open/close
-- ordering, so it is archived but never rolled up.
WHERE last IS NOT NULL AND recovered = 0
GROUP BY exchange, symbol, session_date, currency;

-- Feed health. Written by the ingestor, read by alerting. Separate from the
-- data path so a gap in quotes is still visible when quotes stop entirely.
CREATE TABLE IF NOT EXISTS amd.feed_health
(
    source          LowCardinality(String),
    exchange        LowCardinality(String),
    observed_at     DateTime64(3, 'UTC'),
    -- as_of minus received_at, the metric that degrades before anyone complains.
    lag_seconds     Int64,
    quotes_received UInt32,
    errors          UInt32,
    -- Sequence gaps detected in the window. Always zero for REST sources.
    gaps            UInt32 DEFAULT 0
)
ENGINE = MergeTree
PARTITION BY toYYYYMM(observed_at)
ORDER BY (source, exchange, observed_at)
TTL toDateTime(observed_at) + INTERVAL 90 DAY;
