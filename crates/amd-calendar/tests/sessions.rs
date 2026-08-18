use amd_calendar::{
    SessionState, VENUES, is_trading_day, session_date, session_state, staleness_seconds,
    unverified, venue,
};
use amd_core::ExchangeCode;
use time::macros::datetime;

#[test]
fn ngx_session_spans_the_extended_window() {
    let ngx = venue(ExchangeCode::Ngx);
    // WAT is UTC+1 year-round.
    assert_eq!(
        session_state(datetime!(2026-08-18 08:30 UTC), ngx).unwrap(),
        SessionState::Open
    ); // 09:30
    assert_eq!(
        session_state(datetime!(2026-08-18 14:30 UTC), ngx).unwrap(),
        SessionState::Open
    ); // 15:30
    assert_eq!(
        session_state(datetime!(2026-08-18 07:30 UTC), ngx).unwrap(),
        SessionState::PreOpen
    ); // 08:30
    assert_eq!(
        session_state(datetime!(2026-08-18 15:30 UTC), ngx).unwrap(),
        SessionState::Closed
    ); // 16:30
}

#[test]
fn ngx_1530_wat_is_open_after_the_2026_extension() {
    // Guards the 2026-04-27 window extension against a silent regression:
    // under the old 14:00 close this would have been Closed.
    let ngx = venue(ExchangeCode::Ngx);
    assert_eq!(
        session_state(datetime!(2026-08-18 14:30 UTC), ngx).unwrap(),
        SessionState::Open
    );
}

#[test]
fn weekends_are_closed() {
    let ngx = venue(ExchangeCode::Ngx);
    let saturday = datetime!(2026-08-22 11:00 UTC);
    assert!(!is_trading_day(saturday, ngx).unwrap());
    assert_eq!(session_state(saturday, ngx).unwrap(), SessionState::Closed);
}

#[test]
fn egypt_trades_sunday_to_thursday() {
    let egx = venue(ExchangeCode::Egx);
    assert!(is_trading_day(datetime!(2026-08-23 09:00 UTC), egx).unwrap()); // Sunday
    assert!(is_trading_day(datetime!(2026-08-20 09:00 UTC), egx).unwrap()); // Thursday
    assert!(!is_trading_day(datetime!(2026-08-21 09:00 UTC), egx).unwrap()); // Friday
    assert!(!is_trading_day(datetime!(2026-08-22 09:00 UTC), egx).unwrap()); // Saturday
}

#[test]
fn session_date_uses_exchange_local_time_not_utc() {
    // 23:30 UTC is already the next day in Nairobi (UTC+3) and Lagos (UTC+1).
    assert_eq!(
        session_date(datetime!(2026-08-18 23:30 UTC), venue(ExchangeCode::Nse)).unwrap(),
        "2026-08-19"
    );
    assert_eq!(
        session_date(datetime!(2026-08-18 23:30 UTC), venue(ExchangeCode::Ngx)).unwrap(),
        "2026-08-19"
    );
    assert_eq!(
        session_date(datetime!(2026-08-18 12:00 UTC), venue(ExchangeCode::Ngx)).unwrap(),
        "2026-08-18"
    );
    // Casablanca observes DST, which naive offset arithmetic would get wrong.
    assert_eq!(
        session_date(datetime!(2026-08-18 23:30 UTC), venue(ExchangeCode::Cse)).unwrap(),
        "2026-08-19"
    );
}

#[test]
fn every_declared_timezone_resolves_against_the_tzdb() {
    // Catches a typo'd IANA identifier at test time rather than in production
    // at 09:00 WAT on a Monday.
    for v in VENUES {
        session_state(datetime!(2026-08-18 12:00 UTC), v)
            .unwrap_or_else(|e| panic!("{} ({}): {e}", v.code, v.timezone));
    }
}

#[test]
fn registry_is_exhaustive_over_exchange_codes() {
    for code in ExchangeCode::ALL {
        let v = venue(code);
        assert_eq!(v.code, code);
        assert!(!v.sessions.is_empty(), "{code} has no session windows");
        assert_ne!(v.trading_days, 0, "{code} has no trading days");
        for w in v.sessions {
            assert!(w.open_min < w.close_min, "{code} has an inverted window");
            assert!(w.close_min <= 24 * 60, "{code} closes past midnight");
        }
    }
}

#[test]
fn registry_is_honest_about_what_has_not_been_verified() {
    let un: Vec<_> = unverified().map(|v| v.code).collect();
    assert!(
        !un.contains(&ExchangeCode::Ngx),
        "NGX was checked against a primary source"
    );
    assert!(
        !un.is_empty(),
        "the unverified list should not be silently empty"
    );
}

#[test]
fn staleness_is_clamped_on_clock_skew() {
    let now = datetime!(2026-08-18 12:00:00 UTC);
    assert_eq!(
        staleness_seconds(datetime!(2026-08-18 11:45:00 UTC), now),
        900
    );
    assert_eq!(
        staleness_seconds(datetime!(2026-08-18 12:05:00 UTC), now),
        0
    );
}
