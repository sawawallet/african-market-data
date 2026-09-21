//! Registry of African venues.
//!
//! `sessions_verified` is the honest field. Entries marked false carry
//! best-effort session times that nobody has checked against the venue's own
//! published schedule. They are wrong often enough to matter, and correcting
//! one is the single most useful contribution this project takes.
//!
//! Both venues verified so far were wrong the same way: the window began at the
//! pre-open or auction-call time rather than at continuous trading. Secondary
//! listings quote "trading hours" as the whole schedule including order entry,
//! and that is the number that gets copied. `sessions` means the window in
//! which trades execute continuously — an auction call belongs to `PreOpen`.
//! Check the venue's own rulebook for the phase names before flipping a flag.

use amd_core::{Currency, ExchangeCode};

/// A continuous trading window in exchange-local wall-clock time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub open_min: u16,
    pub close_min: u16,
}

impl Window {
    const fn new(oh: u16, om: u16, ch: u16, cm: u16) -> Self {
        Window {
            open_min: oh * 60 + om,
            close_min: ch * 60 + cm,
        }
    }
}

/// Weekday bitmask. Monday is bit 0, running through to Sunday at bit 6.
///
/// Written as explicit shifts rather than binary literals: Sunday sitting at
/// the top of the mask rather than beside Monday makes hand-written literals
/// easy to miscount by one digit, and the failure is silent.
pub const MON: u8 = 1 << 0;
pub const TUE: u8 = 1 << 1;
pub const WED: u8 = 1 << 2;
pub const THU: u8 = 1 << 3;
pub const FRI: u8 = 1 << 4;
pub const SAT: u8 = 1 << 5;
pub const SUN: u8 = 1 << 6;

pub const MON_FRI: u8 = MON | TUE | WED | THU | FRI;
/// Sunday through Thursday, as traded in Egypt.
pub const SUN_THU: u8 = SUN | MON | TUE | WED | THU;

#[derive(Debug, Clone)]
pub struct Venue {
    pub code: ExchangeCode,
    pub name: &'static str,
    pub countries: &'static [&'static str],
    pub currency: Currency,
    /// IANA identifier, resolved against the tzdb at construction.
    pub timezone: &'static str,
    pub sessions: &'static [Window],
    /// Bitmask of trading weekdays; Monday is bit 0.
    pub trading_days: u8,
    /// Whether `sessions` has been checked against the venue's own schedule.
    pub sessions_verified: bool,
    /// Where the session times came from, so the next person can re-check.
    pub sessions_source: Option<&'static str>,
}

const NGX_SESSIONS: &[Window] = &[Window::new(9, 0, 16, 0)];
const JSE_SESSIONS: &[Window] = &[Window::new(9, 0, 17, 0)];
// 09:30-10:00 is the Pre-Open Period, not trading: orders are entered but
// nothing executes until the 10:00 Opening. Modelling it as open reported a
// market as trading half an hour before it was.
const GSE_SESSIONS: &[Window] = &[Window::new(10, 0, 15, 0)];
// 09:00-09:30:59 is the Open Auction Call — order entry into an auction, not
// continuous trading, which the rules call Regular Trading and start at 09:31.
// The 08:45 Pre-Trading phase sits earlier still.
const NSE_SESSIONS: &[Window] = &[Window::new(9, 31, 15, 0)];
const EGX_SESSIONS: &[Window] = &[Window::new(10, 0, 14, 30)];
// BRVM's published schedule is in UTC, which for Abidjan is also local time
// (Cote d'Ivoire keeps UTC+0 year-round and observes no DST) — so these read
// as wall-clock without conversion. 09:00-09:45 is pre-opening and the 09:45
// fixing opens the book; after continuous trading ends at 14:00 the venue runs
// pre-closing, a closing fixing, then last-price trading to 15:00. Only the
// phase BRVM itself calls "cotation continue" is modelled here.
const BRVM_SESSIONS: &[Window] = &[Window::new(9, 45, 14, 0)];
const CSE_SESSIONS: &[Window] = &[Window::new(9, 30, 15, 20)];
const SEM_SESSIONS: &[Window] = &[Window::new(9, 0, 13, 30)];
// Botswana runs two regular trading sessions either side of a ten-minute
// intra-day auction, so this is the first venue here with more than one
// window. The auction between them is deliberately not covered: nothing trades
// continuously at 12:00, and claiming otherwise is the error this field exists
// to avoid. Auction-call and post-close phases sit outside both windows.
const BSE_SESSIONS: &[Window] = &[Window::new(10, 25, 11, 55), Window::new(12, 5, 13, 20)];
const LUSE_SESSIONS: &[Window] = &[Window::new(10, 0, 14, 0)];
// Pre-Opening 09:00-09:29 and the 09:30 Opening Auction precede continuous
// trading, which the DSE's own schedule starts at 09:31 and closes at 16:00.
const DSE_SESSIONS: &[Window] = &[Window::new(9, 31, 16, 0)];
const USE_SESSIONS: &[Window] = &[Window::new(9, 30, 12, 0)];
// ZSE publishes Pre-Open 09:00-09:30, Market Open 09:30-13:00, Post-Close
// 13:00-14:30. The old 09:00-15:30 took in the pre-open at one end and ran an
// hour past the post-close at the other.
const ZSE_SESSIONS: &[Window] = &[Window::new(9, 30, 13, 0)];
// MSE's market schedule: Pre-Open 09:00-09:30, Open 09:30-14:30, Close
// 14:30-15:00. The registry opened during the pre-open and shut half an hour
// before the market actually did.
const MSE_SESSIONS: &[Window] = &[Window::new(9, 30, 14, 30)];
const RSE_SESSIONS: &[Window] = &[Window::new(9, 0, 12, 0)];
// BVMT runs two seasonal schedules and publishes both as its own avis: winter
// (from 1 September) has continuous trading 09:00-14:00, summer (1 July-31
// August) 09:00-12:00. The 08:30 pre-open is order entry, and the 14:05 closing
// fixing plus 14:05-14:15 last-price trading follow the close. The old close of
// 14:10 fell inside that closing sequence, not at the end of continuous
// trading. Only the standard winter window is modelled; the summer window and
// the Ramadan variant are not.
const BVMT_SESSIONS: &[Window] = &[Window::new(9, 0, 14, 0)];

pub static VENUES: &[Venue] = &[
    Venue {
        code: ExchangeCode::Ngx,
        name: "Nigerian Exchange",
        countries: &["NG"],
        currency: Currency::NGN,
        timezone: "Africa/Lagos",
        sessions: NGX_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "NGX trading window extended to 09:00-16:00 WAT, effective 2026-04-27",
        ),
    },
    Venue {
        code: ExchangeCode::Jse,
        name: "Johannesburg Stock Exchange",
        countries: &["ZA"],
        currency: Currency::ZAR,
        timezone: "Africa/Johannesburg",
        sessions: JSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    // NSX trades on JSE infrastructure and tracks its schedule.
    Venue {
        code: ExchangeCode::Nsx,
        name: "Namibian Stock Exchange",
        countries: &["NA"],
        currency: Currency::new(*b"NAD"),
        timezone: "Africa/Windhoek",
        sessions: JSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Gse,
        name: "Ghana Stock Exchange",
        countries: &["GH"],
        currency: Currency::GHS,
        timezone: "Africa/Accra",
        sessions: GSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "GSE Approved Trading Rules (Equities), Trading Session: Pre-Open 9:30-10:00,              Opening 10:00, Continuous Auction 10:00-15:00, Closing 15:00",
        ),
    },
    Venue {
        code: ExchangeCode::Nse,
        name: "Nairobi Securities Exchange",
        countries: &["KE"],
        currency: Currency::KES,
        timezone: "Africa/Nairobi",
        sessions: NSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "NSE Trading Rules for Equity Securities, rule 6.1.4: Pre-Trading 08:45-08:59:59,              Open Auction Call 09:00-09:30:59, Regular Trading 09:31-15:00, Close 15:00",
        ),
    },
    Venue {
        code: ExchangeCode::Egx,
        name: "Egyptian Exchange",
        countries: &["EG"],
        currency: Currency::EGP,
        timezone: "Africa/Cairo",
        sessions: EGX_SESSIONS,
        trading_days: SUN_THU,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Brvm,
        name: "Bourse Regionale des Valeurs Mobilieres",
        countries: &["CI", "SN", "BJ", "BF", "ML", "NE", "TG", "GW"],
        currency: Currency::new(*b"XOF"),
        timezone: "Africa/Abidjan",
        sessions: BRVM_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "BRVM horaires de cotation (times published in UTC = Abidjan local): \
             pre-ouverture 09:00-09:45, fixing d'ouverture 09:45, cotation continue \
             09:45-14:00, pre-cloture 14:00-14:30, fixing de cloture 14:30, cotation au \
             dernier cours 14:30-15:00, cloture 15:00",
        ),
    },
    Venue {
        code: ExchangeCode::Cse,
        name: "Casablanca Stock Exchange",
        countries: &["MA"],
        currency: Currency::new(*b"MAD"),
        timezone: "Africa/Casablanca",
        sessions: CSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Sem,
        name: "Stock Exchange of Mauritius",
        countries: &["MU"],
        currency: Currency::new(*b"MUR"),
        timezone: "Indian/Mauritius",
        sessions: SEM_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Bse,
        name: "Botswana Stock Exchange",
        countries: &["BW"],
        currency: Currency::new(*b"BWP"),
        timezone: "Africa/Gaborone",
        sessions: BSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "BSE published market hours: Pre-trading 10:00-10:10, Opening Auction Call \
             10:10-10:25, Regular Trading 1 10:25-11:55, Intra-Day Auction 11:55-12:05, \
             Regular Trading 2 12:05-13:20, Closing Auction Call 13:20-13:30, close 14:00",
        ),
    },
    Venue {
        code: ExchangeCode::Luse,
        name: "Lusaka Securities Exchange",
        countries: &["ZM"],
        currency: Currency::new(*b"ZMW"),
        timezone: "Africa/Lusaka",
        sessions: LUSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Dse,
        name: "Dar es Salaam Stock Exchange",
        countries: &["TZ"],
        currency: Currency::new(*b"TZS"),
        timezone: "Africa/Dar_es_Salaam",
        sessions: DSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "DSE Circular 75, Tenth Schedule (Rules 173(3), 197(1)), in force 2 June 2025: \
             Pre-Opening 09:00-09:29, Opening Auction 09:30, Continuous 09:31-16:00, Close 16:00",
        ),
    },
    Venue {
        code: ExchangeCode::Use,
        name: "Uganda Securities Exchange",
        countries: &["UG"],
        currency: Currency::new(*b"UGX"),
        timezone: "Africa/Kampala",
        sessions: USE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Zse,
        name: "Zimbabwe Stock Exchange",
        countries: &["ZW"],
        // ZWG, not USD. The exchange's own market panel reports Turnover and
        // Market Cap in ZWG and does not mention USD at all. The USD entry
        // looks like a conflation with VFEX, Zimbabwe's separate
        // USD-denominated exchange, which is not in this registry.
        currency: Currency::new(*b"ZWG"),
        timezone: "Africa/Harare",
        sessions: ZSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "ZSE site footer \"Trading Hours\": Pre-Open 09:00-09:30, Market Open 09:30-13:00, Post-Close 13:00-14:30 (zse.co.zw)",
        ),
    },
    Venue {
        code: ExchangeCode::Mse,
        name: "Malawi Stock Exchange",
        countries: &["MW"],
        currency: Currency::new(*b"MWK"),
        timezone: "Africa/Blantyre",
        sessions: MSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "MSE published market schedule: Pre-Open 09:00-09:30, Open 09:30-14:30, \
             Close 14:30-15:00",
        ),
    },
    Venue {
        code: ExchangeCode::Rse,
        name: "Rwanda Stock Exchange",
        countries: &["RW"],
        currency: Currency::new(*b"RWF"),
        timezone: "Africa/Kigali",
        sessions: RSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "RSE trading systems page: open-outcry session on the floor during formal \
             trading hours 09:00-12:00, alongside an OTC market. No auction or pre-open \
             phase is published, so unlike the electronic venues here there is none to \
             exclude — the window is the whole formal session, and the existing value \
             was already correct",
        ),
    },
    Venue {
        code: ExchangeCode::Bvmt,
        name: "Bourse de Tunis",
        countries: &["TN"],
        currency: Currency::new(*b"TND"),
        timezone: "Africa/Tunis",
        sessions: BVMT_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: true,
        sessions_source: Some(
            "BVMT avis \"Horaire de cotation hiver\" 2026 (from 1 Sep): continu 09:00-14:00, fixing 14:05; tunis-stockexchange.com/horaires",
        ),
    },
];

pub fn venue(code: ExchangeCode) -> &'static Venue {
    VENUES
        .iter()
        .find(|v| v.code == code)
        .expect("every ExchangeCode variant has a registry entry; see registry_is_exhaustive test")
}

/// Venues whose session times still need checking against the venue itself.
pub fn unverified() -> impl Iterator<Item = &'static Venue> {
    VENUES.iter().filter(|v| !v.sessions_verified)
}
