//! Two layers of test.
//!
//! The fixture tests are the contract: they run offline, in CI, and pin the
//! response shape this adapter was written against. The `live` tests are
//! `#[ignore]` because a public CI run must never depend on — or hammer — a
//! free third-party service. Run them deliberately with:
//!
//! ```text
//! cargo test -p amd-adapters -- --ignored
//! ```

use amd_adapters::http::HttpClient;
use amd_adapters::kwayisi::KwayisiAdapter;
use amd_adapters::{Adapter, AdapterError};
use amd_core::{Currency, DEFAULT_SCALE, DelayClass, ExchangeCode, Price};
use serde_json::Value;

const LIVE_FIXTURE: &str = include_str!("fixtures/kwayisi-live.json");
const DETAIL_FIXTURE: &str = include_str!("fixtures/kwayisi-mtngh.json");

#[test]
fn live_fixture_has_the_shape_this_adapter_expects() {
    let rows: Vec<Value> = serde_json::from_str(LIVE_FIXTURE).expect("fixture is a JSON array");
    assert!(!rows.is_empty(), "GSE board should not be empty");
    for row in &rows {
        assert!(
            row.get("name").and_then(Value::as_str).is_some(),
            "row missing ticker: {row}"
        );
        assert!(
            row.get("price").and_then(Value::as_f64).is_some(),
            "row missing price: {row}"
        );
    }
    // No timestamp field anywhere — the reason this adapter imputes `as_of`.
    let first = &rows[0];
    for key in [
        "time",
        "timestamp",
        "asOf",
        "as_of",
        "updated",
        "updated_at",
        "date",
    ] {
        assert!(
            first.get(key).is_none(),
            "fixture unexpectedly has a {key} field"
        );
    }
}

#[test]
fn every_fixture_price_survives_fixed_point_conversion() {
    let rows: Vec<Value> = serde_json::from_str(LIVE_FIXTURE).unwrap();
    for row in rows {
        let sym = row["name"].as_str().unwrap();
        let price = row["price"].as_f64().unwrap();
        Price::from_f64(price, Currency::GHS, DEFAULT_SCALE)
            .unwrap_or_else(|e| panic!("{sym} price {price}: {e}"));
        if let Some(change) = row.get("change").and_then(Value::as_f64) {
            Price::from_f64(change, Currency::GHS, DEFAULT_SCALE)
                .unwrap_or_else(|e| panic!("{sym} change {change}: {e}"));
        }
    }
}

#[test]
fn detail_fixture_carries_company_metadata() {
    let d: Value = serde_json::from_str(DETAIL_FIXTURE).unwrap();
    assert_eq!(d["name"].as_str(), Some("MTNGH"));
    // Top-level `name` is the ticker; the company name lives one level down.
    assert!(d["company"]["name"].as_str().is_some());
    assert!(d["shares"].as_f64().is_some());
}

#[tokio::test]
async fn refuses_exchanges_it_does_not_serve() {
    let a = KwayisiAdapter::new(HttpClient::new().unwrap());
    let err = a.quotes(ExchangeCode::Ngx, &[]).await.unwrap_err();
    assert!(matches!(
        err,
        AdapterError::UnsupportedExchange {
            exchange: ExchangeCode::Ngx,
            ..
        }
    ));
}

#[test]
fn declares_its_limitations_honestly() {
    let a = KwayisiAdapter::new(HttpClient::new().unwrap());
    let caps = a.capabilities();
    // The source publishes no timestamps and no history. Saying so lets callers
    // degrade gracefully instead of probing for errors.
    assert!(!caps.timestamps);
    assert!(!caps.history);
    assert!(caps.quotes && caps.instruments);
}

#[tokio::test]
#[ignore = "hits the live kwayisi API"]
async fn live_board_is_fetchable_and_internally_consistent() {
    let a = KwayisiAdapter::new(HttpClient::new().unwrap());
    let quotes = a.quotes(ExchangeCode::Gse, &[]).await.expect("live fetch");
    assert!(
        quotes.len() > 20,
        "expected a full GSE board, got {}",
        quotes.len()
    );

    for q in &quotes {
        assert_eq!(q.instrument.exchange, ExchangeCode::Gse);
        assert!(q.last.is_some(), "{} has no last price", q.instrument);
        // The contract this adapter promises about its own honesty.
        assert!(q.provenance.as_of_imputed);
        assert_eq!(q.provenance.delay, DelayClass::Unknown);
        assert_eq!(q.provenance.source, "kwayisi");
        assert!(
            q.provenance.sequence.is_none(),
            "REST sources carry no sequence"
        );
        // previous_close is derived from an absolute change, so the round trip
        // back to last must be exact.
        if let (Some(last), Some(prev)) = (q.last, q.previous_close) {
            assert_eq!(last.currency, prev.currency);
            assert!(prev.change_bps(last).is_ok() || prev.is_zero());
        }
    }
}

#[tokio::test]
#[ignore = "hits the live kwayisi API"]
async fn live_symbol_filter_narrows_the_board() {
    let a = KwayisiAdapter::new(HttpClient::new().unwrap());
    let wanted = vec!["MTNGH".to_string()];
    let quotes = a
        .quotes(ExchangeCode::Gse, &wanted)
        .await
        .expect("live fetch");
    assert_eq!(quotes.len(), 1);
    assert_eq!(quotes[0].instrument.symbol, "MTNGH");
}

#[tokio::test]
#[ignore = "hits the live kwayisi API"]
async fn live_instrument_detail_resolves_company_name() {
    let a = KwayisiAdapter::new(HttpClient::new().unwrap());
    let inst = a.instrument_detail("MTNGH").await.expect("live fetch");
    assert_eq!(inst.symbol(), "MTNGH");
    assert!(inst.name.is_some());
    assert!(inst.shares_outstanding.unwrap_or(0) > 0);
}
