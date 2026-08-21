import { describe, expect, test } from "bun:test";

import { monthWindow } from "./months";

const MONTHS = [
  "2025-12",
  "2026-01",
  "2026-02",
  "2026-03",
  "2026-04",
  "2026-05",
  "2026-06",
  "2026-07",
  "2026-08",
];

describe("monthWindow", () => {
  test("takes the basis months strictly before the selected month", () => {
    expect(monthWindow(MONTHS, "2026-08", 3)).toEqual(["2026-05", "2026-07"]);
    expect(monthWindow(MONTHS, "2026-08", 6)).toEqual(["2026-02", "2026-07"]);
  });

  test("clamps to the ledger's first month", () => {
    expect(monthWindow(MONTHS, "2026-02", 6)).toEqual(["2025-12", "2026-01"]);
    expect(monthWindow(MONTHS, "2026-01", 12)).toEqual(["2025-12", "2025-12"]);
  });

  test("has no window at the first month or off the range", () => {
    expect(monthWindow(MONTHS, "2025-12", 6)).toBeNull();
    expect(monthWindow(MONTHS, "2020-01", 6)).toBeNull();
  });
});
