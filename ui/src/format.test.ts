import { describe, expect, test } from "bun:test";

import {
  fmt,
  fmtCode,
  monthName,
  monthShort,
  pctLabel,
  ratio,
  windowLabel,
} from "./format";

describe("fmt", () => {
  test("uses currency symbols with thousands separators", () => {
    expect(fmt(1234.5, "BRL")).toBe("R$ 1,234.50");
    expect(fmt(3290.1, "USD")).toBe("$ 3,290.10");
  });

  test("marks negatives with a minus sign before the symbol", () => {
    expect(fmt(-306, "USD")).toBe("−$ 306.00");
  });

  test("can round to whole units", () => {
    expect(fmt(14200.4, "BRL", 0)).toBe("R$ 14,200");
  });

  test("falls back to the commodity code when there is no symbol", () => {
    expect(fmt(30, "VACHR")).toBe("VACHR 30.00");
  });
});

describe("fmtCode", () => {
  test("puts the code after the amount", () => {
    expect(fmtCode(-20, "USD")).toBe("−20.00 USD");
    expect(fmtCode(618, "EUR")).toBe("618.00 EUR");
  });
});

describe("month labels", () => {
  test("renders full and short month names", () => {
    expect(monthName("2026-08")).toBe("August 2026");
    expect(monthShort("2025-12")).toBe("Dec");
  });
});

describe("windowLabel", () => {
  test("shows the span of the averaging window", () => {
    expect(windowLabel(["2025-12", "2026-01"])).toBe("Dec – Jan 2026");
  });

  test("collapses a single-month window", () => {
    expect(windowLabel(["2026-01", "2026-01"])).toBe("Jan 2026");
  });

  test("dashes out a missing window", () => {
    expect(windowLabel(null)).toBe("—");
  });
});

describe("pctLabel", () => {
  test("rounds ratios to whole percentages", () => {
    expect(pctLabel(0.9)).toBe("90%");
    expect(pctLabel(3.3333)).toBe("333%");
  });

  test("dashes out a missing ratio", () => {
    expect(pctLabel(null)).toBe("—");
  });
});

describe("ratio", () => {
  test("drops the decimal once a share is big enough not to need it", () => {
    expect(ratio(0.4231)).toBe("42%");
    expect(ratio(1)).toBe("100%");
  });

  test("keeps one decimal in the single digits", () => {
    expect(ratio(0.042)).toBe("4.2%");
  });

  test("does not print a decimal that is only a zero", () => {
    expect(ratio(0.0999)).toBe("10%");
    expect(ratio(0.05)).toBe("5%");
  });

  // The bug this exists to kill: two different ratios both floored to
  // "<1%" read as the same number, and the sentence holding them reads
  // as if it repeats itself.
  test("keeps two figures below one percent rather than a bucket", () => {
    expect(ratio(0.000123)).toBe("0.012%");
    expect(ratio(0.000456)).toBe("0.046%");
    expect(ratio(0.0000004)).toBe("0.00004%");
  });

  test("says zero only when there is nothing there", () => {
    expect(ratio(0)).toBe("0%");
  });

  test("carries a true minus sign, like every other amount", () => {
    expect(ratio(-0.052)).toBe("\u22125.2%");
  });

  test("dashes out a missing ratio", () => {
    expect(ratio(null)).toBe("\u2014");
  });
});
