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

test("performance periods clamp calendar months and use the prior year end for YTD", async () => {
  const {performanceStart}=await import("./investments");
  expect(performanceStart("2026-03-31","1M")).toBe("2026-02-28");
  expect(performanceStart("2024-03-31","1M")).toBe("2024-02-29");
  expect(performanceStart("2026-09-08","3M")).toBe("2026-06-08");
  expect(performanceStart("2026-09-08","YTD")).toBe("2025-12-31");
  expect(performanceStart("2024-02-29","1Y")).toBe("2023-02-28");
  expect(performanceStart("2026-09-08","All")).toBeNull();
});

test("chart paths preserve missing valuations as gaps and space points by elapsed time", async () => {
  const {performanceChart}=await import("./investments");
  const chart=performanceChart([
    {date:"2026-01-01",value:100,net_flows:0,gain:0},
    {date:"2026-01-02",value:null,net_flows:null,gain:null},
    {date:"2026-01-11",value:200,net_flows:50,gain:50},
  ],100);
  expect(chart.valuePath.match(/M/g)).toHaveLength(2);
  expect(chart.points[1]!.x).toBeCloseTo(100);
  expect(chart.points[2]!.x).toBe(1000);
  expect(chart.capitalPath.match(/M/g)).toHaveLength(2);
});
