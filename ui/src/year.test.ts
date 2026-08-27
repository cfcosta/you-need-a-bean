import { describe, expect, test } from "bun:test";

import type { YearGroup, YearMonth } from "./api";
import { foldGroups, yearRibbon, yearStrip } from "./year";

const AXIS = ["2025-11", "2025-12", "2026-01", "2026-02"];

const months = (
  totals: number[],
  priors: (number | null)[] = totals.map(() => null),
): YearMonth[] =>
  AXIS.map((month, i) => ({
    month,
    total: totals[i] ?? 0,
    prior: priors[i] ?? null,
  }));

const group = (
  name: string,
  monthly: number[],
  typical: number | null,
  prior: number | null = null,
): YearGroup => ({
  name,
  monthly,
  typical,
  prior,
  total: monthly.reduce((s, v) => s + v, 0),
});

describe("yearRibbon", () => {
  test("measures every month against the busiest one", () => {
    const r = yearRibbon(months([100, 400, 200, 0]), 150);
    expect(r.max).toBe(400);
    expect(r.months.map((m) => m.height)).toEqual([0.25, 1, 0.5, 0]);
  });

  test("puts the usual month on the same scale as the bars", () => {
    const r = yearRibbon(months([100, 400, 200, 0]), 150);
    expect(r.typicalHeight).toBe(0.375);
  });

  test("scales last year against this one so the tick stays inside", () => {
    // Last November dwarfs every month of this year: the ribbon has to
    // shrink to hold it, or the tick lands outside its own bar.
    const r = yearRibbon(months([100, 400, 200, 0], [800, 0, 0, 0]), 150);
    expect(r.max).toBe(800);
    expect(r.months[0]?.priorHeight).toBe(1);
    expect(r.months[0]?.height).toBe(0.125);
    // A month with no year before it has no tick at all, not a zero one.
    expect(yearRibbon(months([100]), null).months[0]?.priorHeight).toBeNull();
  });

  test("reads a year with no spending as flat, not as a divide by zero", () => {
    const r = yearRibbon(months([0, 0, 0, 0]), null);
    expect(r.max).toBe(0);
    expect(r.typicalHeight).toBeNull();
    expect(r.months.every((m) => m.height === 0)).toBe(true);
  });

  test("calls out the months that cost more than a usual one", () => {
    const r = yearRibbon(months([100, 400, 200, 0]), 150);
    expect(r.months.map((m) => m.above)).toEqual([false, true, true, false]);
  });

  test("drops a usual line that only traces the tallest bar", () => {
    // Twelve identical months have a median equal to their maximum, so
    // the line would run along the top of every bar and say nothing.
    expect(yearRibbon(months([300, 300, 300, 300]), 300).typicalHeight)
      .toBeNull();
  });

  test("marks where the calendar year turns over", () => {
    const r = yearRibbon(months([1, 1, 1, 1]), 1);
    // The window's own start is a turn too — it is where its year is
    // worth printing.
    expect(r.months.map((m) => m.yearStart)).toEqual([
      true,
      false,
      true,
      false,
    ]);
  });
});

describe("yearStrip", () => {
  test("measures a group against its own biggest month, not the card's", () => {
    // Travel happens once. On the card's scale its row would be a
    // sliver; on its own it says plainly that the year was one trip.
    const s = yearStrip(group("Travel", [0, 0, 3000, 0], 3000), months([0]));
    expect(s.max).toBe(3000);
    expect(s.cells.map((c) => c.height)).toEqual([0, 0, 1, 0]);
  });

  test("keeps every cell under its month", () => {
    // A group that stopped mid-year is short one number, and the strip
    // pads rather than sliding the rest out from under their months.
    const s = yearStrip(group("Gym", [80, 80], 80), months([0, 0, 0, 0]));
    expect(s.cells.map((c) => c.month)).toEqual(AXIS);
    expect(s.cells.map((c) => c.value)).toEqual([80, 80, 0, 0]);
  });

  test("calls out the months that cost more than the group usually does", () => {
    const s = yearStrip(group("Food", [500, 900, 510, 0], 505), months([0]));
    expect(s.cells.map((c) => c.above)).toEqual([false, true, true, false]);
    expect(s.typicalHeight).toBeCloseTo(505 / 900);
  });

  test("drops the usual line from a group paid only once", () => {
    // One payment is its own median and its own maximum: nothing to
    // read the line against.
    const s = yearStrip(group("Tuition", [0, 900, 0, 0], 900), months([0]));
    expect(s.typicalHeight).toBeNull();
    expect(s.cells.map((c) => c.above)).toEqual([false, false, false, false]);
  });

  test("leaves a group that was never paid flat and with no usual line", () => {
    const s = yearStrip(group("Tuition", [0, 0, 0, 0], null), months([0]));
    expect(s.max).toBe(0);
    expect(s.typicalHeight).toBeNull();
    expect(s.cells.every((c) => c.height === 0 && !c.above)).toBe(true);
  });
});

describe("foldGroups", () => {
  test("adds the tail up month by month so it still lines up", () => {
    const folded = foldGroups(
      "3 more groups",
      [group("a", [10, 0, 5, 0], 7), group("b", [1, 2, 0, 0], 1)],
      4,
    );
    expect(folded.monthly).toEqual([11, 2, 5, 0]);
    expect(folded.total).toBe(18);
  });

  test("gives a bag of unrelated groups no usual month", () => {
    // Rent and haircuts have no shared typical cost, so the row gets no
    // reference line rather than a meaningless one.
    expect(foldGroups("x", [group("a", [10], 10)], 1).typical).toBeNull();
  });

  test("carries the year before only when there is one", () => {
    const withPrior = [group("a", [10], 10, 4), group("b", [2], 2, 6)];
    expect(foldGroups("x", withPrior, 1).prior).toBe(10);
    expect(foldGroups("x", [group("a", [10], 10)], 1).prior).toBeNull();
  });
});
