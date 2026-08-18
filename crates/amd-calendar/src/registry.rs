//! Registry of African venues.
//!
//! `sessions_verified` is the honest field. Entries marked false carry
//! best-effort session times that nobody has checked against the venue's own
//! published schedule. They are wrong often enough to matter, and correcting
//! one is the single most useful contribution this project takes.

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
const GSE_SESSIONS: &[Window] = &[Window::new(9, 30, 15, 0)];
const NSE_SESSIONS: &[Window] = &[Window::new(9, 0, 15, 0)];
const EGX_SESSIONS: &[Window] = &[Window::new(10, 0, 14, 30)];
const BRVM_SESSIONS: &[Window] = &[Window::new(9, 0, 15, 0)];
const CSE_SESSIONS: &[Window] = &[Window::new(9, 30, 15, 20)];
const SEM_SESSIONS: &[Window] = &[Window::new(9, 0, 13, 30)];
const BSE_SESSIONS: &[Window] = &[Window::new(9, 0, 13, 0)];
const LUSE_SESSIONS: &[Window] = &[Window::new(10, 0, 14, 0)];
const DSE_SESSIONS: &[Window] = &[Window::new(10, 0, 15, 30)];
const USE_SESSIONS: &[Window] = &[Window::new(9, 30, 12, 0)];
const ZSE_SESSIONS: &[Window] = &[Window::new(9, 0, 15, 30)];
const MSE_SESSIONS: &[Window] = &[Window::new(9, 0, 14, 0)];
const RSE_SESSIONS: &[Window] = &[Window::new(9, 0, 12, 0)];
const BVMT_SESSIONS: &[Window] = &[Window::new(9, 0, 14, 10)];

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
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Nse,
        name: "Nairobi Securities Exchange",
        countries: &["KE"],
        currency: Currency::KES,
        timezone: "Africa/Nairobi",
        sessions: NSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
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
        sessions_verified: false,
        sessions_source: None,
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
        sessions_verified: false,
        sessions_source: None,
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
        sessions_verified: false,
        sessions_source: None,
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
        currency: Currency::USD,
        timezone: "Africa/Harare",
        sessions: ZSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Mse,
        name: "Malawi Stock Exchange",
        countries: &["MW"],
        currency: Currency::new(*b"MWK"),
        timezone: "Africa/Blantyre",
        sessions: MSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Rse,
        name: "Rwanda Stock Exchange",
        countries: &["RW"],
        currency: Currency::new(*b"RWF"),
        timezone: "Africa/Kigali",
        sessions: RSE_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
    },
    Venue {
        code: ExchangeCode::Bvmt,
        name: "Bourse de Tunis",
        countries: &["TN"],
        currency: Currency::new(*b"TND"),
        timezone: "Africa/Tunis",
        sessions: BVMT_SESSIONS,
        trading_days: MON_FRI,
        sessions_verified: false,
        sessions_source: None,
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
