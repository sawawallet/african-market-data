//! Reference data and symbology.

use amd_core::{Currency, ExchangeCode, Instrument, InstrumentId};
use sqlx::PgPool;

use crate::StoreError;

#[derive(Clone)]
pub struct ReferenceStore {
    pool: PgPool,
}

impl ReferenceStore {
    pub fn new(pool: PgPool) -> Self {
        ReferenceStore { pool }
    }

    pub async fn migrate(pool: &PgPool) -> Result<(), StoreError> {
        sqlx::migrate!("../../deploy/postgres/migrations")
            .run(pool)
            .await
            .map_err(|e| StoreError::Migrate(e.to_string()))
    }

    /// Insert or refresh an instrument.
    ///
    /// `last_seen` is bumped on every observation so a delisted instrument is
    /// visibly stale rather than silently identical to an active one.
    pub async fn upsert(&self, inst: &Instrument) -> Result<(), StoreError> {
        sqlx::query!(
            r#"
            INSERT INTO instruments
                (id, exchange, symbol, currency, name, isin, sector, shares_outstanding)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (id) DO UPDATE SET
                name               = COALESCE(EXCLUDED.name, instruments.name),
                isin               = COALESCE(EXCLUDED.isin, instruments.isin),
                sector             = COALESCE(EXCLUDED.sector, instruments.sector),
                shares_outstanding = COALESCE(EXCLUDED.shares_outstanding,
                                              instruments.shares_outstanding),
                last_seen          = now()
            "#,
            inst.id.to_string(),
            inst.id.exchange.as_str(),
            inst.id.symbol,
            inst.currency.as_str(),
            inst.name.as_deref(),
            inst.isin.as_deref(),
            inst.sector.as_deref(),
            inst.shares_outstanding.map(|s| s as i64),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list(&self, exchange: ExchangeCode) -> Result<Vec<Instrument>, StoreError> {
        let rows = sqlx::query!(
            r#"
            SELECT id, exchange, symbol, currency, name, isin, sector, shares_outstanding
            FROM instruments WHERE exchange = $1 ORDER BY symbol
            "#,
            exchange.as_str()
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let currency: Currency = r.currency.parse().ok()?;
                let exchange: ExchangeCode = r.exchange.parse().ok()?;
                Some(Instrument {
                    id: InstrumentId::new(exchange, &r.symbol),
                    currency,
                    name: r.name,
                    isin: r.isin,
                    sector: r.sector,
                    shares_outstanding: r.shares_outstanding.map(|s| s as u64),
                })
            })
            .collect())
    }

    /// Resolve a source's own ticker to our internal instrument id.
    ///
    /// Returns `None` rather than guessing. A wrong mapping silently attributes
    /// one company's prices to another, which is worse than an absent one.
    pub async fn resolve_alias(
        &self,
        source: &str,
        source_symbol: &str,
    ) -> Result<Option<InstrumentId>, StoreError> {
        let row = sqlx::query!(
            "SELECT instrument_id FROM symbol_aliases WHERE source = $1 AND source_symbol = $2",
            source,
            source_symbol
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.and_then(|r| r.instrument_id.parse().ok()))
    }

    pub async fn record_alias(
        &self,
        source: &str,
        source_symbol: &str,
        instrument: &InstrumentId,
        provenance: &str,
    ) -> Result<(), StoreError> {
        sqlx::query!(
            r#"
            INSERT INTO symbol_aliases (source, source_symbol, instrument_id, provenance)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (source, source_symbol) DO UPDATE SET
                instrument_id = EXCLUDED.instrument_id,
                provenance    = EXCLUDED.provenance
            "#,
            source,
            source_symbol,
            instrument.to_string(),
            provenance
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
