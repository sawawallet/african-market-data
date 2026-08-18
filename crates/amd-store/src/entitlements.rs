//! Entitlement enforcement.
//!
//! The check that keeps a free tier from being served licensed real-time data.
//! It runs at the egress boundary against the [`DelayClass`] carried on the
//! datum itself, rather than against an assumption about where the datum came
//! from — because by the time a quote reaches the API it may have arrived via
//! any of several adapters with different entitlements.

use amd_core::{DelayClass, ExchangeCode};
use sqlx::PgPool;
use uuid::Uuid;

use crate::StoreError;

#[derive(Debug, Clone)]
pub struct ApiKey {
    pub id: Uuid,
    pub label: String,
    /// Highest delay class this key may receive.
    pub max_delay_class: DelayClass,
    /// `None` means every venue this deployment serves.
    pub exchanges: Option<Vec<ExchangeCode>>,
    pub rate_limit_rpm: i32,
}

impl ApiKey {
    /// Whether this key may be served a datum of the given class from a venue.
    ///
    /// Both conditions must hold. A key entitled to real-time on JSE is not
    /// thereby entitled to real-time on NGX — venue scoping is how a partial
    /// licence is represented.
    pub fn may_receive(&self, exchange: ExchangeCode, delay: DelayClass) -> bool {
        let venue_ok = self
            .exchanges
            .as_ref()
            .is_none_or(|list| list.contains(&exchange));
        venue_ok && self.max_delay_class.permits(delay)
    }
}

#[derive(Clone)]
pub struct Entitlements {
    pool: PgPool,
}

impl Entitlements {
    pub fn new(pool: PgPool) -> Self {
        Entitlements { pool }
    }

    /// Resolve a presented key by its hash.
    ///
    /// The caller hashes; this never sees the raw key, so a log line or an
    /// error message cannot leak one.
    pub async fn lookup(&self, key_hash: &str) -> Result<Option<ApiKey>, StoreError> {
        let row = sqlx::query!(
            r#"
            SELECT id, label, max_delay_class, exchanges, rate_limit_rpm
            FROM api_keys
            WHERE key_hash = $1 AND revoked_at IS NULL
            "#,
            key_hash
        )
        .fetch_optional(&self.pool)
        .await?;

        let Some(r) = row else { return Ok(None) };

        let max_delay_class = match r.max_delay_class.as_str() {
            "realtime" => DelayClass::Realtime,
            "delayed" => DelayClass::Delayed,
            "eod" => DelayClass::Eod,
            // An unrecognised value must fail closed. Defaulting to a
            // permissive class here would be the exact bug this module exists
            // to prevent.
            other => {
                tracing::error!(key = %r.id, value = %other, "unknown delay class, denying");
                return Ok(None);
            }
        };

        let exchanges = r.exchanges.map(|list| {
            list.iter()
                .filter_map(|s| s.parse().ok())
                .collect::<Vec<_>>()
        });

        Ok(Some(ApiKey {
            id: r.id,
            label: r.label,
            max_delay_class,
            exchanges,
            rate_limit_rpm: r.rate_limit_rpm,
        }))
    }
}
