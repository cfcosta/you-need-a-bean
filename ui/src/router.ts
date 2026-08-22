/**
 * The URL is the app's state of record: which page, which month, and the
 * view options on top of them. Everything here is pure string work — the
 * App owns `history`, this module only says what a URL means and what a
 * state should look like as a URL.
 */

export type Page = "budget" | "reports";

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
}

/** A fully decided route — what the app actually renders. */
export interface RouteState {
  page: Page;
  month: string;
  basis: number;
  cur: string;
  cat: string | null;
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
  const [head, tail] = loc.pathname.split("/").filter(Boolean);
  const q = new URLSearchParams(loc.search);
  const page: Page = head === "reports" ? "reports" : "budget";
  const basis = Number(q.get("basis"));
  return {
    page,
    month: tail != null && MONTH.test(tail) ? tail : null,
    basis: BASES.includes(basis) ? basis : null,
    cur: q.get("cur"),
    cat: page === "budget" ? q.get("cat") : null,
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
  };
}

/** The canonical URL for a state: defaults stay out of it, so the plain
 * budget page is just `/`. */
export function routeUrl(s: RouteState, d: RouteDefaults): string {
  const q = new URLSearchParams();
  if (s.basis !== DEFAULT_BASIS) q.set("basis", String(s.basis));
  if (s.cur !== (d.operating_currencies[0] ?? "USD")) q.set("cur", s.cur);
  if (s.page === "budget" && s.cat != null) q.set("cat", s.cat);
  const path =
    s.page === "reports"
      ? "/reports"
      : s.month === d.default_month
        ? "/"
        : `/budget/${s.month}`;
  const query = q.toString();
  return query === "" ? path : `${path}?${query}`;
}
