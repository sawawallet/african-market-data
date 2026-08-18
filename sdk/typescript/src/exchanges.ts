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
    sessions: [{ open: "09:30", close: "15:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  NSE: {
    code: "NSE",
    name: "Nairobi Securities Exchange",
    countries: ["KE"],
    currency: "KES",
    timezone: "Africa/Nairobi",
    sessions: [{ open: "09:00", close: "15:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
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
    sessions: [{ open: "09:00", close: "15:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
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
    sessions: [{ open: "09:00", close: "13:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
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
    sessions: [{ open: "10:00", close: "15:30" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
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
    currency: "USD",
    timezone: "Africa/Harare",
    sessions: [{ open: "09:00", close: "15:30" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  MSE: {
    code: "MSE",
    name: "Malawi Stock Exchange",
    countries: ["MW"],
    currency: "MWK",
    timezone: "Africa/Blantyre",
    sessions: [{ open: "09:00", close: "14:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
  },
  RSE: {
    code: "RSE",
    name: "Rwanda Stock Exchange",
    countries: ["RW"],
    currency: "RWF",
    timezone: "Africa/Kigali",
    sessions: [{ open: "09:00", close: "12:00" }],
    tradingDays: MON_TO_FRI,
    sessionsVerified: false,
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
