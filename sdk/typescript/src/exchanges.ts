/**
 * Registry of African exchanges.
 *
 * `sessionsVerified` is the honest bit. Entries marked false carry best-effort
 * session times that nobody has checked against the venue's own published
 * schedule. They are wrong often enough to matter, and correcting one is the
 * single most useful contribution this project takes — see CONTRIBUTING.md.
 */
import type { Exchange, ExchangeCode } from "./types.js";

const MON_TO_FRI = [1, 2, 3, 4, 5] as const;

export const EXCHANGES: Readonly<Record<ExchangeCode, Exchange>> = Object.freeze({
  NGX: {
    code: "NGX",
    name: "Nigerian Exchange",
    countries: ["NG"],
    currency: "NGN",
    timezone: "Africa/Lagos",
    // Extended from 09:30-14:00 to a seven-hour continuous window on
    // 27 April 2026, to deepen liquidity and widen investor access.
    sessions: [{ open: "09:00", close: "16:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource: "NGX trading window extension, effective 2026-04-27",
  },
  JSE: {
    code: "JSE",
    name: "Johannesburg Stock Exchange",
    countries: ["ZA"],
    currency: "ZAR",
    timezone: "Africa/Johannesburg",
    sessions: [{ open: "09:00", close: "17:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  NSX: {
    code: "NSX",
    name: "Namibian Stock Exchange",
    countries: ["NA"],
    currency: "NAD",
    timezone: "Africa/Windhoek",
    // NSX trades on JSE infrastructure and tracks its schedule.
    sessions: [{ open: "09:00", close: "17:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  GSE: {
    code: "GSE",
    name: "Ghana Stock Exchange",
    countries: ["GH"],
    currency: "GHS",
    timezone: "Africa/Accra",
    // 09:30-10:00 is the Pre-Open Period; nothing executes until 10:00.
    sessions: [{ open: "10:00", close: "15:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "GSE Approved Trading Rules (Equities), Trading Session: Pre-Open 9:30-10:00, Opening 10:00, Continuous Auction 10:00-15:00, Closing 15:00",
  },
  NSE: {
    code: "NSE",
    name: "Nairobi Securities Exchange",
    countries: ["KE"],
    currency: "KES",
    timezone: "Africa/Nairobi",
    // 09:00-09:30:59 is the Open Auction Call, not continuous trading.
    sessions: [{ open: "09:31", close: "15:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "NSE Trading Rules for Equity Securities, rule 6.1.4: Pre-Trading 08:45-08:59:59, Open Auction Call 09:00-09:30:59, Regular Trading 09:31-15:00, Close 15:00",
  },
  EGX: {
    code: "EGX",
    name: "Egyptian Exchange",
    countries: ["EG"],
    currency: "EGP",
    timezone: "Africa/Cairo",
    sessions: [{ open: "10:00", close: "14:30" }],
    // Egypt trades Sunday to Thursday.
    tradingDays: [0, 1, 2, 3, 4],
    sessionsVerified: false,
  },
  BRVM: {
    code: "BRVM",
    name: "Bourse Régionale des Valeurs Mobilières",
    countries: ["CI", "SN", "BJ", "BF", "ML", "NE", "TG", "GW"],
    currency: "XOF",
    timezone: "Africa/Abidjan",
    // Only the phase BRVM calls "cotation continue" is modelled.
    sessions: [{ open: "09:45", close: "14:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "BRVM horaires de cotation (times published in UTC = Abidjan local): pre-ouverture 09:00-09:45, fixing d'ouverture 09:45, cotation continue 09:45-14:00, pre-cloture 14:00-14:30, fixing de cloture 14:30, cotation au dernier cours 14:30-15:00, cloture 15:00",
  },
  CSE: {
    code: "CSE",
    name: "Casablanca Stock Exchange",
    countries: ["MA"],
    currency: "MAD",
    timezone: "Africa/Casablanca",
    sessions: [{ open: "09:30", close: "15:20" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  SEM: {
    code: "SEM",
    name: "Stock Exchange of Mauritius",
    countries: ["MU"],
    currency: "MUR",
    timezone: "Indian/Mauritius",
    sessions: [{ open: "09:00", close: "13:30" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  BSE: {
    code: "BSE",
    name: "Botswana Stock Exchange",
    countries: ["BW"],
    currency: "BWP",
    timezone: "Africa/Gaborone",
    // Two regular sessions either side of an intra-day auction.
    sessions: [
      { open: "10:25", close: "11:55" },
      { open: "12:05", close: "13:20" },
    ],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "BSE published market hours: Pre-trading 10:00-10:10, Opening Auction Call 10:10-10:25, Regular Trading 1 10:25-11:55, Intra-Day Auction 11:55-12:05, Regular Trading 2 12:05-13:20, Closing Auction Call 13:20-13:30, close 14:00",
  },
  LUSE: {
    code: "LUSE",
    name: "Lusaka Securities Exchange",
    countries: ["ZM"],
    currency: "ZMW",
    timezone: "Africa/Lusaka",
    sessions: [{ open: "10:00", close: "14:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  DSE: {
    code: "DSE",
    name: "Dar es Salaam Stock Exchange",
    countries: ["TZ"],
    currency: "TZS",
    timezone: "Africa/Dar_es_Salaam",
    sessions: [{ open: "09:31", close: "16:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "DSE Circular 75, Tenth Schedule (Rules 173(3), 197(1)), in force 2 June 2025: Pre-Opening 09:00-09:29, Opening Auction 09:30, Continuous 09:31-16:00, Close 16:00",
  },
  USE: {
    code: "USE",
    name: "Uganda Securities Exchange",
    countries: ["UG"],
    currency: "UGX",
    timezone: "Africa/Kampala",
    sessions: [{ open: "09:30", close: "12:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  ZSE: {
    code: "ZSE",
    name: "Zimbabwe Stock Exchange",
    countries: ["ZW"],
    currency: "ZWG",
    timezone: "Africa/Harare",
    sessions: [{ open: "09:30", close: "13:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "ZSE published trading hours (Mon-Fri, excluding public holidays): Pre-Open 09:00-09:30, Market Open 09:30-13:00, Post-Close 13:00-14:30",
  },
  MSE: {
    code: "MSE",
    name: "Malawi Stock Exchange",
    countries: ["MW"],
    currency: "MWK",
    timezone: "Africa/Blantyre",
    sessions: [{ open: "09:30", close: "14:30" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "MSE published market schedule: Pre-Open 09:00-09:30, Open 09:30-14:30, Close 14:30-15:00",
  },
  RSE: {
    code: "RSE",
    name: "Rwanda Stock Exchange",
    countries: ["RW"],
    currency: "RWF",
    timezone: "Africa/Kigali",
    sessions: [{ open: "09:00", close: "12:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: true,
    sessionsSource:
      "RSE trading systems page: open-outcry session on the floor during formal trading hours 09:00-12:00; no auction or pre-open phase is published",
  },
  BVMT: {
    code: "BVMT",
    name: "Bourse de Tunis",
    countries: ["TN"],
    currency: "TND",
    timezone: "Africa/Tunis",
    sessions: [{ open: "09:00", close: "14:10" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
});

export const EXCHANGE_CODES = Object.keys(EXCHANGES) as ExchangeCode[];

export function getExchange(code: ExchangeCode): Exchange {
  const ex = EXCHANGES[code];
  if (!ex) throw new Error(`unknown exchange: ${code}`);
  return ex;
}

/** Exchanges whose session times still need checking against the venue. */
export function unverifiedExchanges(): Exchange[] {
  return EXCHANGE_CODES.map((c) => EXCHANGES[c]).filter((e) => !e.sessionsVerified);
}
