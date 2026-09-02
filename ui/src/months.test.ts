import { describe, expect, test } from "bun:test";

import { addMonths, monthWindow } from "./months";

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

describe("addMonths", () => {
  test("counts forward and back across a year end", () => {
    expect(addMonths("2026-08", 5)).toBe("2027-01");
    expect(addMonths("2026-01", -1)).toBe("2025-12");
    expect(addMonths("2026-11", 0)).toBe("2026-11");
    expect(addMonths("2024-02", 25)).toBe("2026-03");
  });
});
