use amd_core::instrument::{ExchangeCode, Instrument, InstrumentId};
use amd_core::money::Currency;
use amd_core::provenance::{DelayClass, Provenance};
use amd_core::quote::Quote;
use time::OffsetDateTime;
use time::macros::datetime;

#[test]
fn instrument_ids_normalise_case_and_round_trip() {
    let id: InstrumentId = "ngx:mtnn".parse().unwrap();
    assert_eq!(id.exchange, ExchangeCode::Ngx);
    assert_eq!(id.symbol, "MTNN");
    assert_eq!(id.to_string(), "NGX:MTNN");
    assert_eq!("NGX:MTNN".parse::<InstrumentId>().unwrap(), id);
}

#[test]
fn malformed_instrument_ids_are_rejected() {
    assert!("MTNN".parse::<InstrumentId>().is_err());
    assert!("NGX:".parse::<InstrumentId>().is_err());
    assert!("XXX:MTNN".parse::<InstrumentId>().is_err());
}

#[test]
fn entitlement_ordering_never_leaks_upward() {
    use DelayClass::*;
    // A real-time entitlement subsumes everything below it.
    assert!(Realtime.permits(Delayed));
    assert!(Realtime.permits(Eod));
    assert!(Delayed.permits(Eod));
    // But never the reverse — this is the check that keeps a free tier from
    // being served licensed real-time data.
    assert!(!Delayed.permits(Realtime));
    assert!(!Eod.permits(Realtime));
    assert!(!Eod.permits(Delayed));
    // Unknown proves nothing, so it satisfies nothing but itself.
    assert!(!Unknown.permits(Eod));
    assert!(!Realtime.permits(Unknown));
    assert!(Unknown.permits(Unknown));
}

#[test]
fn lag_is_clamped_on_clock_skew() {
    let as_of = datetime!(2026-08-18 12:00:00 UTC);
    let late = Provenance::stamped(
        "jse-mitch",
        as_of,
        datetime!(2026-08-18 12:00:30 UTC),
        DelayClass::Realtime,
    );
    assert_eq!(late.lag_seconds(), 30);
    // A datum stamped in the future means skew, not time travel.
    let skewed = Provenance::stamped(
        "jse-mitch",
        as_of,
        datetime!(2026-08-18 11:59:30 UTC),
        DelayClass::Realtime,
    );
    assert_eq!(skewed.lag_seconds(), 0);
}

#[test]
fn imputed_provenance_is_flagged() {
    let p = Provenance::imputed("kwayisi", OffsetDateTime::now_utc(), DelayClass::Unknown);
    assert!(p.as_of_imputed);
    assert_eq!(p.as_of, p.received_at);
    assert!(!p.recovered);
}

#[test]
fn recovered_data_is_marked() {
    // MITCH snapshot recovery does not preserve original timestamps, so
    // anything reconstructed must be excluded from microstructure analytics.
    let p = Provenance::stamped(
        "jse-mitch",
        datetime!(2026-08-18 12:00:00 UTC),
        datetime!(2026-08-18 12:00:00 UTC),
        DelayClass::Realtime,
    )
    .with_sequence(65_001)
    .recovered();
    assert!(p.recovered);
    assert_eq!(p.sequence, Some(65_001));
}

#[test]
fn crossed_books_are_detectable() {
    let inst = Instrument::new(ExchangeCode::Jse, "NPN", Currency::ZAR);
    let prov = Provenance::imputed("test", OffsetDateTime::now_utc(), DelayClass::Unknown);
    let mut q = Quote::new(inst.id.clone(), prov);
    assert_eq!(q.is_crossed(), None);

    q.bid = Some(amd_core::Price::parse("100.50", Currency::ZAR, 8).unwrap());
    q.ask = Some(amd_core::Price::parse("100.60", Currency::ZAR, 8).unwrap());
    assert_eq!(q.is_crossed(), Some(false));

    q.bid = Some(amd_core::Price::parse("100.70", Currency::ZAR, 8).unwrap());
    assert_eq!(q.is_crossed(), Some(true));
}

#[test]
fn quotes_serialise_without_losing_precision() {
    let inst = Instrument::new(ExchangeCode::Gse, "MTNGH", Currency::GHS);
    let prov = Provenance::imputed(
        "kwayisi",
        datetime!(2026-08-18 12:00:00 UTC),
        DelayClass::Unknown,
    );
    let mut q = Quote::new(inst.id.clone(), prov);
    q.last = Some(amd_core::Price::parse("7.03", Currency::GHS, 8).unwrap());
    q.previous_close = Some(amd_core::Price::parse("6.96", Currency::GHS, 8).unwrap());

    let json = serde_json::to_string(&q).unwrap();
    let back: Quote = serde_json::from_str(&json).unwrap();
    assert_eq!(back, q);
    assert_eq!(back.change_bps().unwrap().unwrap(), 100); // +1.00%
}
