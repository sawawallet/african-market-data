//! Persistence.
//!
//! Two stores, split by access pattern rather than by taste:
//!
//! - **ClickHouse** holds the tick archive and the rollups. Append-only, wide,
//!   scanned analytically.
//! - **Postgres** holds reference data, symbology and entitlements. Small,
//!   mutable, relational, read on every request.

pub mod clickhouse_store;
pub mod entitlements;
pub mod reference;

pub use clickhouse_store::{ClickhouseStore, QuoteRow};
pub use entitlements::{ApiKey, Entitlements};
pub use reference::ReferenceStore;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("clickhouse: {0}")]
    Clickhouse(String),
    #[error("postgres: {0}")]
    Postgres(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migrate(String),
    #[error("{0}")]
    Invalid(String),
}
