import { expect, test } from "bun:test";
import type { Position } from "./api";
import { selectHoldings, holdingPrice, quoteAge } from "./investments";

const holding = (currency: string, overrides: Partial<Position> = {}): Position => ({
  currency, label: currency, class: "equity", units: 10, price: 20, value: 200,
  share: .5, basis: 100, gain: 100, ret: 1, accounts: 1, postings: 2,
  first: "2026-01-01", last: "2026-09-01", price_date: "2026-09-01",
  locations: [{account:"Assets:Broker",label:"Long-term account",units:10}], ...overrides,
});

test("holding filters search names and accounts without changing portfolio order", () => {
  const rows = [holding("BOND", {class:null}), holding("FUND", {label:"Broad market fund"})];
  expect(selectHoldings(rows,"broad", "all", "value").map(p=>p.currency)).toEqual(["FUND"]);
  expect(selectHoldings(rows,"long-term", "all", "value")).toHaveLength(2);
  expect(selectHoldings(rows,"", "null", "value").map(p=>p.currency)).toEqual(["BOND"]);
  expect(selectHoldings(rows,"", '"equity"', "value").map(p=>p.currency)).toEqual(["FUND"]);
  expect(rows.map(p=>p.currency)).toEqual(["BOND","FUND"]);
});

test("unknown cost never sorts as a zero gain", () => {
  const rows = [holding("UNKNOWN",{gain:null}),holding("LOSS",{gain:-10}),holding("GAIN",{gain:20})];
  expect(selectHoldings(rows,"","all","gain").map(p=>p.currency)).toEqual(["GAIN","LOSS","UNKNOWN"]);
});

test("small prices retain meaningful precision and quote age uses calendar days", () => {
  expect(holdingPrice(.0001,"USD")).toContain("0.0001");
  expect(holdingPrice(.000000001,"USD")).not.toMatch(/0\.0+$/);
  expect(quoteAge("2026-09-01","2026-09-08")).toBe(7);
  expect(quoteAge(null,"2026-09-08")).toBeNull();
});
