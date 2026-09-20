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

#[test]
fn gse_pre_open_period_is_not_trading() {
    // GSE rules: Pre-Open 09:30-10:00, Opening 10:00, Continuous Auction
    // 10:00-15:00. Accra is UTC+0 year-round, so local time is UTC.
    let gse = venue(ExchangeCode::Gse);
    assert_eq!(
        session_state(datetime!(2026-08-18 09:45 UTC), gse).unwrap(),
        SessionState::PreOpen
    ); // inside the pre-open period: orders entered, nothing executes
    assert_eq!(
        session_state(datetime!(2026-08-18 10:00 UTC), gse).unwrap(),
        SessionState::Open
    ); // the opening
    assert_eq!(
        session_state(datetime!(2026-08-18 14:59 UTC), gse).unwrap(),
        SessionState::Open
    );
    assert_eq!(
        session_state(datetime!(2026-08-18 15:00 UTC), gse).unwrap(),
        SessionState::Closed
    ); // closing
}

#[test]
fn nse_open_auction_call_is_not_regular_trading() {
    // NSE rule 6.1.4: Open Auction Call 09:00-09:30:59, Regular Trading from
    // 09:31. EAT is UTC+3 year-round.
    let nse = venue(ExchangeCode::Nse);
    assert_eq!(
        session_state(datetime!(2026-08-18 06:15 UTC), nse).unwrap(),
        SessionState::PreOpen
    ); // 09:15 EAT — auction call, not yet continuous
    assert_eq!(
        session_state(datetime!(2026-08-18 06:30 UTC), nse).unwrap(),
        SessionState::PreOpen
    ); // 09:30 EAT — still the auction call
    assert_eq!(
        session_state(datetime!(2026-08-18 06:31 UTC), nse).unwrap(),
        SessionState::Open
    ); // 09:31 EAT — Regular Trading begins
    assert_eq!(
        session_state(datetime!(2026-08-18 12:00 UTC), nse).unwrap(),
        SessionState::Closed
    ); // 15:00 EAT — close
}

#[test]
fn verified_venues_cite_their_source() {
    // A flag without a citation cannot be re-checked, which defeats the point
    // of having the flag.
    for v in VENUES.iter().filter(|v| v.sessions_verified) {
        assert!(
            v.sessions_source.is_some(),
            "{:?} is marked verified but cites no source",
            v.code
        );
    }
}

#[test]
fn dse_continuous_trading_starts_after_the_opening_auction() {
    // DSE Circular 75 (in force 2 June 2025): Pre-Opening 09:00-09:29, Opening
    // Auction 09:30, Continuous 09:31-16:00. EAT is UTC+3 year-round.
    let dse = venue(ExchangeCode::Dse);
    assert_eq!(
        session_state(datetime!(2026-08-18 06:15 UTC), dse).unwrap(),
        SessionState::PreOpen
    ); // 09:15 EAT — pre-opening
    assert_eq!(
        session_state(datetime!(2026-08-18 06:30 UTC), dse).unwrap(),
        SessionState::PreOpen
    ); // 09:30 EAT — the opening auction itself
    assert_eq!(
        session_state(datetime!(2026-08-18 06:31 UTC), dse).unwrap(),
        SessionState::Open
    ); // 09:31 EAT — continuous begins
    assert_eq!(
        session_state(datetime!(2026-08-18 12:30 UTC), dse).unwrap(),
        SessionState::Open
    ); // 15:30 EAT — under the old 15:30 close this was wrongly Closed
    assert_eq!(
        session_state(datetime!(2026-08-18 13:00 UTC), dse).unwrap(),
        SessionState::Closed
    ); // 16:00 EAT — close
}

#[test]
fn bse_has_two_windows_around_the_intraday_auction() {
    // Botswana is the one venue here that trades in two separate continuous
    // blocks. Africa/Gaborone is UTC+2 year-round.
    let bse = venue(ExchangeCode::Bse);
    assert_eq!(
        bse.sessions.len(),
        2,
        "BSE should carry two trading windows"
    );
    assert_eq!(
        session_state(datetime!(2026-08-18 08:20 UTC), bse).unwrap(),
        SessionState::PreOpen
    ); // 10:20 — opening auction call, not yet trading
    assert_eq!(
        session_state(datetime!(2026-08-18 08:30 UTC), bse).unwrap(),
        SessionState::Open
    ); // 10:30 — regular trading 1
    assert_eq!(
        session_state(datetime!(2026-08-18 10:00 UTC), bse).unwrap(),
        SessionState::Closed
    ); // 12:00 — intra-day auction: nothing trades continuously
    assert_eq!(
        session_state(datetime!(2026-08-18 10:30 UTC), bse).unwrap(),
        SessionState::Open
    ); // 12:30 — regular trading 2
    assert_eq!(
        session_state(datetime!(2026-08-18 11:30 UTC), bse).unwrap(),
        SessionState::Closed
    ); // 13:30 — closing auction call has begun
}

#[test]
fn brvm_opens_at_the_fixing_not_at_pre_opening() {
    // BRVM publishes in UTC, and Abidjan is UTC+0 year-round with no DST, so
    // the published times are also local wall-clock.
    let brvm = venue(ExchangeCode::Brvm);
    assert_eq!(
        session_state(datetime!(2026-08-18 09:30 UTC), brvm).unwrap(),
        SessionState::PreOpen
    ); // 09:30 — pre-ouverture
    assert_eq!(
        session_state(datetime!(2026-08-18 09:45 UTC), brvm).unwrap(),
        SessionState::Open
    ); // 09:45 — fixing d'ouverture opens continuous trading
    assert_eq!(
        session_state(datetime!(2026-08-18 13:59 UTC), brvm).unwrap(),
        SessionState::Open
    );
    assert_eq!(
        session_state(datetime!(2026-08-18 14:00 UTC), brvm).unwrap(),
        SessionState::Closed
    ); // 14:00 — continuous ends; pre-cloture and last-price are not continuous
}

// ---------------------------------------------------------------------------
// Registry invariants.
//
// Eleven venues still carry unchecked times, and correcting one is a change to
// a struct literal made by someone who knows the exchange rather than the code.
// These guard the shapes that are easy to get wrong by hand and silent when
// wrong: a window that ends before it starts is simply never open, and nothing
// else in the system would complain.
// ---------------------------------------------------------------------------

/// Minutes in a day. A window is expressed as minutes past local midnight.
const DAY: u16 = 24 * 60;

#[test]
fn every_window_opens_before_it_closes() {
    for v in VENUES {
        for w in v.sessions {
            assert!(
                w.open_min < w.close_min,
                "{:?} has a window that opens at {} and closes at {} — it can never be open",
                v.code,
                w.open_min,
                w.close_min
            );
        }
    }
}

#[test]
fn every_window_falls_inside_a_day() {
    for v in VENUES {
        for w in v.sessions {
            assert!(
                w.close_min <= DAY,
                "{:?} has a window closing at minute {}, past midnight — sessions do not \
                 currently span days, so this is a typo rather than an overnight venue",
                v.code,
                w.close_min
            );
        }
    }
}

#[test]
fn multi_window_venues_are_ordered_and_disjoint() {
    // Botswana trades in two blocks either side of an intra-day auction. Any
    // venue with more than one window must list them in order and leave a real
    // gap: overlapping windows would mean the break is not a break, and
    // out-of-order ones make `earliest_open` — which decides PreOpen vs Closed
    // — read from the wrong window.
    for v in VENUES {
        for pair in v.sessions.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            assert!(
                a.close_min <= b.open_min,
                "{:?} lists windows out of order or overlapping: {}-{} then {}-{}",
                v.code,
                a.open_min,
                a.close_min,
                b.open_min,
                b.close_min
            );
        }
    }
}

#[test]
fn every_venue_trades_on_at_least_one_weekday() {
    // A zero mask parses fine and silently reports the venue closed forever.
    for v in VENUES {
        assert!(
            v.trading_days != 0,
            "{:?} has an empty trading_days mask and would never open",
            v.code
        );
    }
}

#[test]
fn every_venue_has_at_least_one_session() {
    for v in VENUES {
        assert!(
            !v.sessions.is_empty(),
            "{:?} has no trading window at all",
            v.code
        );
    }
}
