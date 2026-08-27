import { describe, expect, test } from "bun:test";

import type { RouteDefaults, RouteState } from "./router";
import { parseRoute, resolveRoute, routeUrl } from "./router";

const DEFAULTS: RouteDefaults = {
  months: ["2025-12", "2026-01", "2026-02", "2026-03", "2026-04", "2026-05"],
  default_month: "2026-05",
  operating_currencies: ["USD", "EUR"],
};

/** A `location`-alike from a URL, the way the browser hands it to us. */
const at = (url: string) => {
  const u = new URL(url, "http://ledger.test");
  return { pathname: u.pathname, search: u.search };
};

describe("parseRoute", () => {
  test("reads the page and month off the path", () => {
    expect(parseRoute(at("/"))).toEqual({
      page: "budget",
      month: null,
      basis: null,
      cur: null,
      cat: null,
      acct: null,
    });
    expect(parseRoute(at("/budget")).month).toBeNull();
    expect(parseRoute(at("/budget/2026-02")).month).toBe("2026-02");
    expect(parseRoute(at("/reports")).page).toBe("reports");
  });

  test("shrugs off paths that are not ours", () => {
    expect(parseRoute(at("/budget/last-tuesday")).month).toBeNull();
    expect(parseRoute(at("/budget/2026-2")).month).toBeNull();
    expect(parseRoute(at("/nonsense")).page).toBe("budget");
  });

  test("reads the view options off the query", () => {
    const r = parseRoute(at("/budget/2026-02?basis=12&cur=EUR&cat=Expenses%3AFun"));
    expect(r.basis).toBe(12);
    expect(r.cur).toBe("EUR");
    expect(r.cat).toBe("Expenses:Fun");
  });

  test("drops options the app cannot honour", () => {
    expect(parseRoute(at("/?basis=7")).basis).toBeNull();
    expect(parseRoute(at("/?basis=six")).basis).toBeNull();
    // Reports has no category pane, so a stray cat is not ours to keep.
    expect(parseRoute(at("/reports?cat=Expenses%3AFun")).cat).toBeNull();
    expect(
      parseRoute(at("/account/Assets%3ACash?cat=Expenses%3AFun")).cat,
    ).toBeNull();
  });

  test("reads an account off the path", () => {
    const r = parseRoute(at("/account/Assets%3AUS%3ABofA%3AChecking"));
    expect(r.page).toBe("account");
    expect(r.acct).toBe("Assets:US:BofA:Checking");
    expect(r.month).toBeNull();
    expect(parseRoute(at("/account/Assets%3ACash/2026-02")).month).toBe(
      "2026-02",
    );
    // An account page with no account on it is not a page.
    expect(parseRoute(at("/account")).page).toBe("budget");
    expect(parseRoute(at("/reports")).acct).toBeNull();
  });
});

describe("resolveRoute", () => {
  test("fills in the ledger's defaults", () => {
    expect(resolveRoute(parseRoute(at("/")), DEFAULTS)).toEqual({
      page: "budget",
      month: "2026-05",
      basis: 6,
      cur: "USD",
      cat: null,
      acct: null,
    });
  });

  test("keeps what the URL asked for when the ledger has it", () => {
    const r = resolveRoute(
      parseRoute(at("/budget/2026-01?basis=3&cur=EUR&cat=Expenses%3AFun")),
      DEFAULTS,
    );
    expect(r).toEqual({
      page: "budget",
      month: "2026-01",
      basis: 3,
      cur: "EUR",
      cat: "Expenses:Fun",
      acct: null,
    });
  });

  test("keeps the account the path named", () => {
    const r = resolveRoute(
      parseRoute(at("/account/Assets%3ACash/2026-01")),
      DEFAULTS,
    );
    expect(r).toEqual({
      page: "account",
      month: "2026-01",
      basis: 6,
      cur: "USD",
      cat: null,
      acct: "Assets:Cash",
    });
  });

  test("falls back when the URL points outside the ledger", () => {
    expect(resolveRoute(parseRoute(at("/budget/1999-01")), DEFAULTS).month).toBe(
      "2026-05",
    );
    expect(resolveRoute(parseRoute(at("/?cur=BRL")), DEFAULTS).cur).toBe("USD");
  });
});

describe("routeUrl", () => {
  const state = (over: Partial<RouteState> = {}): RouteState => ({
    page: "budget",
    month: "2026-05",
    basis: 6,
    cur: "USD",
    cat: null,
    acct: null,
    ...over,
  });

  test("leaves the defaults out", () => {
    expect(routeUrl(state(), DEFAULTS)).toBe("/");
    expect(routeUrl(state({ page: "reports" }), DEFAULTS)).toBe("/reports");
  });

  test("spells out everything else", () => {
    expect(routeUrl(state({ month: "2026-01" }), DEFAULTS)).toBe(
      "/budget/2026-01",
    );
    expect(routeUrl(state({ basis: 12, cur: "EUR" }), DEFAULTS)).toBe(
      "/?basis=12&cur=EUR",
    );
    expect(routeUrl(state({ cat: "Expenses:Fun" }), DEFAULTS)).toBe(
      "/?cat=Expenses%3AFun",
    );
  });

  test("spells the account into the path", () => {
    expect(
      routeUrl(
        state({ page: "account", acct: "Assets:US:BofA:Checking" }),
        DEFAULTS,
      ),
    ).toBe("/account/Assets%3AUS%3ABofA%3AChecking");
    expect(
      routeUrl(
        state({ page: "account", acct: "Assets:Cash", month: "2026-01" }),
        DEFAULTS,
      ),
    ).toBe("/account/Assets%3ACash/2026-01");
  });

  test("has nowhere to send an account page with no account", () => {
    expect(routeUrl(state({ page: "account" }), DEFAULTS)).toBe("/");
  });

  test("drops the category on the page that has no inspector", () => {
    expect(routeUrl(state({ page: "reports", cat: "Expenses:Fun" }), DEFAULTS)).toBe(
      "/reports",
    );
  });

  test("round-trips through the parser", () => {
    for (const s of [
      state(),
      state({ page: "reports", basis: 3 }),
      state({ month: "2025-12", cur: "EUR", cat: "Expenses:Fun" }),
      state({ page: "account", acct: "Assets:Cash" }),
      state({ page: "account", acct: "Assets:Cash", month: "2026-01" }),
    ]) {
      expect(resolveRoute(parseRoute(at(routeUrl(s, DEFAULTS))), DEFAULTS)).toEqual(
        s,
      );
    }
  });
});
