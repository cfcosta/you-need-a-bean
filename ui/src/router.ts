/**
 * The URL is the app's state of record: which page, which month, and the
 * view options on top of them. Everything here is pure string work — the
 * App owns `history`, this module only says what a URL means and what a
 * state should look like as a URL.
 */

export type Page = "home" | "budget" | "reports" | "account" | "liabilities";

/** The bases the month picker offers. */
export const BASES = [3, 6, 12];

export const DEFAULT_BASIS = 6;

/** What a URL asked for; `null` wherever it stayed quiet. */
export interface Route {
  page: Page;
  month: string | null;
  basis: number | null;
  cur: string | null;
  cat: string | null;
  /** The account whose register is on screen; only one page has one. */
  acct: string | null;
}

/** A fully decided route — what the app actually renders. */
export interface RouteState {
  page: Page;
  month: string;
  basis: number;
  cur: string;
  cat: string | null;
  acct: string | null;
}

/** The parts of the summary a route is measured against. */
export interface RouteDefaults {
  months: string[];
  default_month: string;
  operating_currencies: string[];
}

const MONTH = /^\d{4}-\d{2}$/;

/** What a location means, without consulting the ledger. */
export function parseRoute(loc: {
  pathname: string;
  search: string;
}): Route {
  const [head, ...rest] = loc.pathname.split("/").filter(Boolean);
  const q = new URLSearchParams(loc.search);
  // An account name is full of colons, so it travels encoded — and it
  // takes the segment the month sits in everywhere else, which pushes
  // the month one along.
  const acct =
    head === "account" && rest[0] != null
      ? decodeURIComponent(rest[0])
      : null;
  const page: Page =
    (head == null && !q.has("cat")) || head === "home" ? "home" :
    head === "reports"
      ? "reports"
      : head === "liabilities"
        ? "liabilities"
        : acct != null
          ? "account"
          : "budget";
  const month = acct != null ? rest[1] : rest[0];
  const basis = Number(q.get("basis"));
  return {
    page,
    month: month != null && MONTH.test(month) ? month : null,
    basis: BASES.includes(basis) ? basis : null,
    cur: q.get("cur"),
    cat: page === "budget" ? q.get("cat") : null,
    acct,
  };
}

/** The route with the ledger's defaults filled in for anything missing
 * or out of range. */
export function resolveRoute(r: Route, d: RouteDefaults): RouteState {
  const fallbackCur = d.operating_currencies[0] ?? "USD";
  return {
    page: r.page,
    month:
      r.month != null && d.months.includes(r.month) ? r.month : d.default_month,
    basis: r.basis ?? DEFAULT_BASIS,
    cur:
      r.cur != null && d.operating_currencies.includes(r.cur)
        ? r.cur
        : fallbackCur,
    cat: r.cat,
    acct: r.acct,
  };
}

/** The canonical URL for a state: defaults stay out of it, so the plain
 * budget page is just `/`. */
export function routeUrl(s: RouteState, d: RouteDefaults): string {
  const q = new URLSearchParams();
  if (s.basis !== DEFAULT_BASIS) q.set("basis", String(s.basis));
  if (s.cur !== (d.operating_currencies[0] ?? "USD")) q.set("cur", s.cur);
  if (s.page === "budget" && s.cat != null) q.set("cat", s.cat);
  const query = q.toString();
  const path = pagePath(s, d);
  return query === "" ? path : `${path}?${query}`;
}

/** The path half of the URL: which page, and which month of it. */
function pagePath(s: RouteState, d: RouteDefaults): string {
  if (s.page === "home") return "/";
  if (s.page === "reports") return "/reports";
  if (s.page === "liabilities") return "/liabilities";
  // An account page with no account is nowhere; the budget is home.
  if (s.page === "account" && s.acct != null) {
    const at = `/account/${encodeURIComponent(s.acct)}`;
    return s.month === d.default_month ? at : `${at}/${s.month}`;
  }
  return s.month === d.default_month ? "/budget" : `/budget/${s.month}`;
}
