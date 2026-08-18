use std::cmp::Ordering;

use amd_core::money::{Currency, MoneyError, Price};

fn ghs() -> Currency {
    Currency::GHS
}

#[test]
fn parses_exactly() {
    assert_eq!(Price::parse("35.42", ghs(), 2).unwrap().minor, 3542);
    assert_eq!(Price::parse("0.14", ghs(), 4).unwrap().minor, 1400);
    assert_eq!(
        Price::parse("-1.5", Currency::NGN, 4).unwrap().minor,
        -15000
    );
    assert_eq!(Price::parse("78", ghs(), 2).unwrap().minor, 7800);
    assert_eq!(
        Price::parse("1,234.56", Currency::NGN, 2).unwrap().minor,
        123456
    );
    assert_eq!(Price::parse(".5", Currency::NGN, 2).unwrap().minor, 50);
}

#[test]
fn refuses_to_silently_drop_precision() {
    assert!(matches!(
        Price::parse("1.23456", ghs(), 2),
        Err(MoneyError::PrecisionLoss {
            actual: 5,
            scale: 2,
            ..
        })
    ));
    assert!(Price::parse("abc", ghs(), 2).is_err());
    assert!(Price::parse("", ghs(), 2).is_err());
    assert!(Price::parse("1.2.3", ghs(), 4).is_err());
}

#[test]
fn the_float_problem_this_module_exists_to_avoid() {
    assert_ne!(0.1_f64 + 0.2_f64, 0.3_f64);
    let sum = Price::parse("0.1", ghs(), 8)
        .unwrap()
        .add(Price::parse("0.2", ghs(), 8).unwrap())
        .unwrap();
    assert_eq!(
        sum.cmp_value(Price::parse("0.3", ghs(), 8).unwrap())
            .unwrap(),
        Ordering::Equal
    );
}

#[test]
fn handles_what_the_kwayisi_api_actually_returns() {
    // Values taken from a live GET /apis/gse/live response.
    assert_eq!(Price::from_f64(0.42, ghs(), 8).unwrap().minor, 42_000_000);
    assert_eq!(
        Price::from_f64(37.0, ghs(), 8).unwrap().minor,
        3_700_000_000
    );
    assert_eq!(Price::from_f64(-0.01, ghs(), 8).unwrap().minor, -1_000_000);
    assert_eq!(
        Price::from_f64(93_050_310_601.5, ghs(), 8).unwrap().minor,
        9_305_031_060_150_000_000
    );
}

#[test]
fn mitch_scale_prices_survive_a_round_trip() {
    // MITCH publishes signed i64 with eight implied decimals. The widest such
    // value must rescale to a common working scale without overflowing, which
    // is why Price holds i128 rather than i64.
    let widest = Price::new(i64::MAX as i128, 8, Currency::ZAR).unwrap();
    let rescaled = widest.rescale(10).unwrap();
    assert_eq!(rescaled.rescale(8).unwrap().minor, i64::MAX as i128);
}

#[test]
fn rescale_rounds_half_away_from_zero() {
    let ngn = Currency::NGN;
    assert_eq!(
        Price::new(12345, 4, ngn).unwrap().rescale(2).unwrap().minor,
        123
    );
    assert_eq!(
        Price::new(12350, 4, ngn).unwrap().rescale(2).unwrap().minor,
        124
    );
    assert_eq!(
        Price::new(-12350, 4, ngn)
            .unwrap()
            .rescale(2)
            .unwrap()
            .minor,
        -124
    );
    assert_eq!(
        Price::new(-12340, 4, ngn)
            .unwrap()
            .rescale(2)
            .unwrap()
            .minor,
        -123
    );
    assert_eq!(
        Price::new(123, 2, ngn).unwrap().rescale(4).unwrap().minor,
        12300
    );
}

#[test]
fn mixed_scales_align_before_arithmetic() {
    let a = Price::new(150, 2, Currency::NGN).unwrap();
    let b = Price::new(12345, 4, Currency::NGN).unwrap();
    let diff = a.sub(b).unwrap();
    assert_eq!(diff.minor, 2655);
    assert_eq!(diff.scale, 4);
}

#[test]
fn currency_mismatch_is_an_error_not_a_coincidence() {
    let a = Price::new(1, 2, Currency::NGN).unwrap();
    let b = Price::new(1, 2, Currency::GHS).unwrap();
    assert!(matches!(a.add(b), Err(MoneyError::CurrencyMismatch(..))));
}

#[test]
fn change_in_basis_points() {
    let prev = Price::parse("0.78", ghs(), 8).unwrap();
    let last = Price::parse("0.77", ghs(), 8).unwrap();
    assert_eq!(prev.change_bps(last).unwrap(), -128);
    assert_eq!(prev.change_bps(prev).unwrap(), 0);
    assert!(matches!(
        Price::zero(ghs()).change_bps(last),
        Err(MoneyError::ZeroBase)
    ));
}

#[test]
fn display_groups_thousands() {
    assert_eq!(
        Price::parse("1234.5", Currency::NGN, 2)
            .unwrap()
            .to_string(),
        "NGN 1,234.50"
    );
    assert_eq!(
        Price::parse("-7.03", ghs(), 2).unwrap().to_string(),
        "-GHS 7.03"
    );
    assert_eq!(
        Price::parse("0.42", ghs(), 2).unwrap().to_string(),
        "GHS 0.42"
    );
    assert_eq!(
        Price::parse("1234567", Currency::NGN, 0)
            .unwrap()
            .to_string(),
        "NGN 1,234,567"
    );
}

#[test]
fn currency_parsing_is_strict() {
    assert_eq!("ngn".parse::<Currency>().unwrap(), Currency::NGN);
    assert!("NG".parse::<Currency>().is_err());
    assert!("NGNX".parse::<Currency>().is_err());
    assert!("N1N".parse::<Currency>().is_err());
}
