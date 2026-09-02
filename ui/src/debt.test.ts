import { describe, expect, test } from "bun:test";

import {
  amortize,
  dayLabel,
  daysBetween,
  noticeText,
  ordinal,
  sliderRange,
} from "./debt";

describe("amortize", () => {
  // The same loan the server projects, so the slider's zero position
  // agrees with the figure the card already shows.
  test("agrees with the server about the loan in the fixture", () => {
    const a = amortize(9777.89, 0.06, 500);
    expect(a).not.toBeNull();
    expect(a!.months).toBe(21);
    expect(a!.interest).toBe(537.69);
    expect(a!.curve).toHaveLength(21);
    expect(a!.curve[0]).toBeCloseTo(9326.78, 2);
    expect(a!.curve[20]).toBe(0);
  });

  test("a debt with nothing owed is paid off already", () => {
    expect(amortize(0, 0.06, 500)).toEqual({ months: 0, interest: 0, curve: [] });
    expect(amortize(-25, 0.06, 500)).toEqual({
      months: 0,
      interest: 0,
      curve: [],
    });
  });

  test("a payment that does not beat the interest never ends", () => {
    expect(amortize(1000, 0.24, 0)).toBeNull();
    expect(amortize(1000, 0.24, 20)).toBeNull();
    expect(amortize(1000, 0.24, 20.01)).not.toBeNull();
  });

  test("gives up on a payoff a century away", () => {
    expect(amortize(1_000_000, 0.1, 8333.34)).toBeNull();
  });

  test("an interest-free debt is just division", () => {
    expect(amortize(1000, 0, 100)).toEqual({
      months: 10,
      interest: 0,
      curve: [900, 800, 700, 600, 500, 400, 300, 200, 100, 0],
    });
  });
});

describe("sliderRange", () => {
  // The slider runs up to about one more payment, on a round number,
  // and never so short that a card's tiny minimum leaves it useless.
  test("runs up to a round number near the payment", () => {
    expect(sliderRange(500)).toEqual({ max: 500, step: 20 });
    expect(sliderRange(384.04)).toEqual({ max: 500, step: 20 });
    expect(sliderRange(57.8)).toEqual({ max: 100, step: 5 });
    expect(sliderRange(2200)).toEqual({ max: 2500, step: 100 });
  });

  test("never shrinks below a useful range", () => {
    expect(sliderRange(20)).toEqual({ max: 50, step: 2 });
    expect(sliderRange(0)).toEqual({ max: 50, step: 2 });
  });
});

describe("ordinal", () => {
  test("spells the day of the month", () => {
    expect([1, 2, 3, 4, 11, 12, 13, 21, 22, 23, 31].map(ordinal)).toEqual([
      "1st",
      "2nd",
      "3rd",
      "4th",
      "11th",
      "12th",
      "13th",
      "21st",
      "22nd",
      "23rd",
      "31st",
    ]);
  });
});

describe("dayLabel", () => {
  test("is the day and the month, nothing more", () => {
    expect(dayLabel("2026-02-20")).toBe("20 Feb");
    expect(dayLabel("2026-03-01")).toBe("1 Mar");
  });
});

describe("daysBetween", () => {
  test("counts the days from one date to another", () => {
    expect(daysBetween("2026-02-01", "2026-02-20")).toBe(19);
    expect(daysBetween("2025-12-01", "2026-02-15")).toBe(76);
    expect(daysBetween("2026-02-20", "2026-02-01")).toBe(-19);
  });
});

describe("noticeText", () => {
  test("says what the ledger noticed, in a sentence each", () => {
    expect(
      noticeText(
        { kind: "missed", account: "Liabilities:Loan:Car", label: "Car Loan", amount: null, day: 15 },
        "USD",
      ),
    ).toBe(
      "Car Loan: nothing paid yet this month, and it usually goes out on the 15th",
    );
    expect(
      noticeText(
        { kind: "growing", account: "Liabilities:Card:Store", label: "Store Card", amount: null, day: null },
        "USD",
      ),
    ).toBe(
      "Store Card: the balance carried from month to month has grown three months running",
    );
    expect(
      noticeText(
        { kind: "overpaid", account: "Liabilities:Card:Old", label: "Old Card", amount: 25, day: null },
        "USD",
      ),
    ).toBe("Old Card is $ 25.00 in credit — a refund or a payment too many");
  });
});
