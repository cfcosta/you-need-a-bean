import { describe, expect, it } from "bun:test";

import { accountBars, monthProgress, stripBars } from "./strip";

const view = (over: Partial<Parameters<typeof stripBars>[0]> = {}) => ({
  income: 0,
  spent: 0,
  typical: null as number | null,
  is_current: false,
  day: 0,
  days_in_month: 31,
  ...over,
});

describe("stripBars", () => {
  it("puts every tile on one scale, so the bars can be compared", () => {
    const b = stripBars(view({ income: 100, spent: 50, typical: 200 }));
    expect(b.income).toBeCloseTo(0.5);
    expect(b.spent).toBeCloseTo(0.25);
    expect(b.typical).toBeCloseTo(1);
  });

  it("draws the net against the same scale, sign kept separately", () => {
    const b = stripBars(view({ income: 100, spent: 40, typical: 200 }));
    expect(b.net).toBeCloseTo(0.3);
    expect(b.negative).toBe(false);
  });

  it("calls a month that spent more than it earned negative", () => {
    const b = stripBars(view({ income: 40, spent: 100, typical: 200 }));
    expect(b.net).toBeCloseTo(0.3);
    expect(b.negative).toBe(true);
  });

  it("scales to whatever is biggest when that is not the typical month", () => {
    const b = stripBars(view({ income: 400, spent: 50, typical: 200 }));
    expect(b.income).toBeCloseTo(1);
    expect(b.typical).toBeCloseTo(0.5);
  });

  it("has no typical mark when no earlier month had payments", () => {
    const b = stripBars(view({ income: 100, spent: 50 }));
    expect(b.typical).toBeNull();
    expect(b.income).toBeCloseTo(1);
  });

  it("survives a month where nothing happened at all", () => {
    const b = stripBars(view());
    expect(b.income).toBe(0);
    expect(b.spent).toBe(0);
    expect(b.net).toBe(0);
    expect(b.typical).toBeNull();
  });

  // A month still running is only part-spent, so the bar says how far
  // through it we are rather than making the reader work it out.
  it("marks how much of a running month has gone", () => {
    const b = stripBars(
      view({
        income: 1,
        spent: 1,
        is_current: true,
        day: 15,
        days_in_month: 30,
      }),
    );
    expect(b.pace).toBeCloseTo(0.5);
  });

  it("leaves a finished month unmarked", () => {
    const b = stripBars(view({ income: 1, spent: 1, day: 31 }));
    expect(b.pace).toBeNull();
  });

  // Spending evenly, this is where a typical month would have you by
  // now — so the gap between the fill and this mark is the whole
  // "ahead or behind" question, drawn.
  it("places the even-pace point along the typical month", () => {
    const b = stripBars(
      view({
        income: 0,
        spent: 30,
        typical: 100,
        is_current: true,
        day: 15,
        days_in_month: 30,
      }),
    );
    expect(b.typical).toBeCloseTo(1);
    expect(b.paceMark).toBeCloseTo(0.5);
  });

  it("has no pace point once the month is complete", () => {
    const b = stripBars(view({ spent: 30, typical: 100 }));
    expect(b.paceMark).toBeNull();
  });

  it("has no pace point without a typical month to pace against", () => {
    const b = stripBars(
      view({ spent: 30, is_current: true, day: 15, days_in_month: 30 }),
    );
    expect(b.paceMark).toBeNull();
  });

  it("never divides by a month with no days", () => {
    const b = stripBars(
      view({ income: 1, is_current: true, day: 3, days_in_month: 0 }),
    );
    expect(b.pace).toBeNull();
  });

  // Income above the scale would run the fill past its track; the scale
  // is the biggest of them, so this can only come of a bad number.
  it("clamps a fill that would overrun its track", () => {
    const b = stripBars(view({ income: -100, spent: 50, typical: 10 }));
    expect(b.income).toBeLessThanOrEqual(1);
    expect(b.income).toBeGreaterThanOrEqual(0);
  });
});

describe("monthProgress", () => {
  it("is how far through a running month we are", () => {
    expect(
      monthProgress({ is_current: true, day: 12, days_in_month: 30 }),
    ).toBeCloseTo(0.4);
  });

  it("is null for a month that has already finished", () => {
    expect(
      monthProgress({ is_current: false, day: 31, days_in_month: 31 }),
    ).toBeNull();
  });

  it("is null when the month has no days to divide by", () => {
    expect(
      monthProgress({ is_current: true, day: 3, days_in_month: 0 }),
    ).toBeNull();
  });
});

describe("accountBars", () => {
  const acct = {
    opening: 1000,
    inflow: 400,
    outflow: 300,
    balance: 1100,
  };

  it("measures the month's flows against the balances they moved", () => {
    const b = accountBars(acct);
    expect(b.balance).toBeCloseTo(1);
    expect(b.opening).toBeCloseTo(1000 / 1100);
    expect(b.inflow).toBeCloseTo(400 / 1100);
    expect(b.outflow).toBeCloseTo(300 / 1100);
  });

  // A credit card runs negative. Length is size; the sign is a colour.
  it("gives an overdrawn account a length and says which way it points", () => {
    const b = accountBars({ ...acct, opening: -2000, balance: -2200 });
    expect(b.opening).toBeCloseTo(2000 / 2200);
    expect(b.balance).toBeCloseTo(1);
    expect(b.openingNegative).toBe(true);
    expect(b.balanceNegative).toBe(true);
  });

  it("has nothing to draw for an account with no balance yet", () => {
    const b = accountBars({
      opening: null,
      inflow: 0,
      outflow: 0,
      balance: null,
    });
    expect(b.opening).toBeNull();
    expect(b.balance).toBeNull();
    expect(b.inflow).toBe(0);
  });

  it("survives an account where every number is zero", () => {
    const b = accountBars({ opening: 0, inflow: 0, outflow: 0, balance: 0 });
    expect(b.inflow).toBe(0);
    expect(b.opening).toBe(0);
  });
});
