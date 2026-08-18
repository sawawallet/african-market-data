-- Reference data and entitlements.
--
-- Postgres rather than ClickHouse because this is small, mutable, relational
-- and read on every request. ClickHouse holds the ticks; this holds the facts
-- about what those ticks are and who may see them.

CREATE TABLE IF NOT EXISTS instruments (
    id                  TEXT PRIMARY KEY,              -- EXCHANGE:SYMBOL
    exchange            TEXT NOT NULL,
    symbol              TEXT NOT NULL,
    currency            CHAR(3) NOT NULL,
    name                TEXT,
    isin                TEXT,
    sector              TEXT,
    shares_outstanding  BIGINT,
    first_seen          TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen           TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (exchange, symbol)
);

CREATE INDEX IF NOT EXISTS instruments_exchange_idx ON instruments (exchange);
-- ISIN is carried as an attribute, never as the key: coverage across these
-- venues is patchy and occasionally wrong.
CREATE INDEX IF NOT EXISTS instruments_isin_idx ON instruments (isin) WHERE isin IS NOT NULL;

-- Symbol resolution across sources.
--
-- The unglamorous hard part. A vendor's ticker, the venue's own ticker and a
-- global vendor's identifier for the same instrument will not match, and there
-- is no ISIN discipline to fall back on. So the mapping is data, maintained by
-- hand where it has to be, and never buried in code.
CREATE TABLE IF NOT EXISTS symbol_aliases (
    source          TEXT NOT NULL,      -- adapter id, e.g. 'kwayisi'
    source_symbol   TEXT NOT NULL,
    instrument_id   TEXT NOT NULL REFERENCES instruments (id) ON DELETE CASCADE,
    -- How the mapping was established, so a wrong one can be traced.
    provenance      TEXT NOT NULL DEFAULT 'manual',
    confirmed_at    TIMESTAMPTZ,
    PRIMARY KEY (source, source_symbol)
);

CREATE INDEX IF NOT EXISTS symbol_aliases_instrument_idx ON symbol_aliases (instrument_id);

-- What each API consumer is entitled to see.
--
-- Enforced at the egress boundary against the delay class carried on the datum
-- itself, so a free tier cannot be served licensed real-time data by accident.
CREATE TABLE IF NOT EXISTS api_keys (
    id              UUID PRIMARY KEY,
    key_hash        TEXT NOT NULL UNIQUE,   -- argon2 of the presented key
    label           TEXT NOT NULL,
    -- Highest delay class this key may receive: 'realtime' | 'delayed' | 'eod'.
    max_delay_class TEXT NOT NULL DEFAULT 'eod',
    -- NULL means every venue the deployment serves.
    exchanges       TEXT[],
    rate_limit_rpm  INTEGER NOT NULL DEFAULT 60,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at      TIMESTAMPTZ,
    CONSTRAINT api_keys_delay_class_valid
        CHECK (max_delay_class IN ('realtime', 'delayed', 'eod', 'unknown'))
);

CREATE INDEX IF NOT EXISTS api_keys_active_idx ON api_keys (key_hash) WHERE revoked_at IS NULL;

-- Which sources this deployment is licensed to redistribute, and under what
-- delay class. An adapter reporting 'realtime' does not by itself grant the
-- right to serve it onward; this table is the record of what was actually
-- signed.
CREATE TABLE IF NOT EXISTS source_licences (
    source          TEXT NOT NULL,
    exchange        TEXT NOT NULL,
    delay_class     TEXT NOT NULL,
    redistributable BOOLEAN NOT NULL DEFAULT false,
    notes           TEXT,
    valid_from      DATE NOT NULL DEFAULT CURRENT_DATE,
    valid_until     DATE,
    PRIMARY KEY (source, exchange)
);
