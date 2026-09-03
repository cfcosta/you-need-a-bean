import { describe, expect, test } from "bun:test";

import type { Debt } from "./api";
import {
  addDays,
  amortize,
  beatenText,
  columns,
  costShares,
  coverLine,
  dayLabel,
  daysBetween,
  dueEvents,
  earnedLine,
  emphasize,
  foreignText,
  hues,
  monthPlan,
  noticeText,
  ordinal,
  payingDown,
  plan,
  rateTone,
  sliderRange,
  sliderStart,
  stripMarks,
  treadmillFacts,
  verdict,
} from "./debt";

/** A debt with nothing on it, for a test to fill in the parts it is about. */
const debt = (over: Partial<Debt>): Debt => ({
  account: "Liabilities:Loan:X",
  label: "X",
  kind: "installment",
  owed: 0,
  balances: {},
  foreign: [],
  limit: null,
  utilisation: null,
  collateral: null,
  peak: 0,
  progress: null,
  rate: null,
  payment: null,
  due_day: null,
  next_due: null,
  principal_paid: 0,
  interest_paid: 0,
  payments: [],
  history: [],
  trail: [],
  payoff: null,
  cycle: null,
  treadmill: null,
  makeup: null,
  ...over,
});

const cleared = { charges: 0, payments: 0, carried: 0, in_full: true };

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

describe("payingDown", () => {
  test("is the loans with a balance and the cards carrying one", () => {
    const loan = debt({ account: "L", label: "Loan", owed: 500, rate: 0.05, payment: 50 });
    const carried = debt({
      account: "C",
      label: "Carried",
      kind: "revolving",
      owed: 300,
      rate: 0.24,
      payment: 30,
      cycle: { charges: 10, payments: 0, carried: 250, in_full: false },
    });
    const done = debt({ account: "D", label: "Done", owed: 0, payment: 50 });
    const clear = debt({ account: "E", label: "Clear", kind: "revolving", owed: 800, cycle: cleared });
    const credit = debt({ account: "F", label: "Credit", kind: "revolving", owed: -40, cycle: cleared });
    expect(payingDown([loan, carried, done, clear, credit])).toEqual([
      { account: "L", label: "Loan", kind: "installment", owed: 500, rate: 0.05, payment: 50 },
      { account: "C", label: "Carried", kind: "revolving", owed: 300, rate: 0.24, payment: 30 },
    ]);
  });

  test("a debt with no rate or payment seen is one with none", () => {
    expect(payingDown([debt({ account: "L", label: "Loan", owed: 500 })])).toEqual([
      { account: "L", label: "Loan", kind: "installment", owed: 500, rate: 0, payment: 0 },
    ]);
  });
});

describe("plan", () => {
  const one = { account: "A", label: "A", kind: "installment" as const };

  test("a single debt is the same arithmetic as amortize", () => {
    expect(plan([{ ...one, owed: 1000, rate: 0, payment: 100 }], 0, "avalanche")).toEqual({
      order: "avalanche",
      months: 10,
      interest: 0,
      steps: [{ account: "A", label: "A", months: 10, interest: 0 }],
    });
    const p = plan([{ ...one, owed: 9777.89, rate: 0.06, payment: 500 }], 0, "snowball");
    expect(p?.months).toBe(21);
    expect(p?.interest).toBe(537.69);
  });

  test("rolls a finished payment into the next debt", () => {
    // B ends after six months; its 50 then joins A's 100.
    const debts = [
      { ...one, owed: 1000, rate: 0, payment: 100 },
      { account: "B", label: "B", kind: "installment" as const, owed: 300, rate: 0, payment: 50 },
    ];
    const p = plan(debts, 0, "avalanche");
    expect(p?.steps.map((s) => [s.account, s.months])).toEqual([
      ["B", 6],
      ["A", 9],
    ]);
    expect(p?.months).toBe(9);
  });

  test("sends the extra to the dearest debt first, or to the smallest", () => {
    const debts = [
      { ...one, owed: 300, rate: 0, payment: 50 },
      { account: "B", label: "B", kind: "installment" as const, owed: 1000, rate: 0.12, payment: 100 },
    ];
    const avalanche = plan(debts, 100, "avalanche");
    expect(avalanche?.steps.map((s) => [s.account, s.months])).toEqual([
      ["B", 6],
      ["A", 6],
    ]);
    expect(avalanche?.interest).toBe(31.12);
    const snowball = plan(debts, 100, "snowball");
    expect(snowball?.steps.map((s) => [s.account, s.months])).toEqual([
      ["A", 2],
      ["B", 6],
    ]);
    expect(snowball?.interest).toBe(37.25);
    expect(snowball?.steps[1]?.interest).toBe(37.25);
  });

  test("gives up when the payments do not beat the interest", () => {
    expect(plan([{ ...one, owed: 1000, rate: 0.24, payment: 20 }], 0, "avalanche")).toBeNull();
    expect(plan([{ ...one, owed: 1000, rate: 0.24, payment: 20 }], 10, "avalanche")).not.toBeNull();
  });

  test("has nothing to plan when nothing is owed", () => {
    expect(plan([], 50, "snowball")).toEqual({
      order: "snowball",
      months: 0,
      interest: 0,
      steps: [],
    });
  });
});

describe("foreignText", () => {
  test("says what a move in the rate would do to the bill", () => {
    expect(
      foreignText([{ code: "EUR", amount: 885.3, converted: 827.67 }], 827.67, "USD"),
    ).toBe("All of it is € 885.30: a 10% move in the euro is ± $ 83 on the bill.");
    expect(
      foreignText([{ code: "GBP", amount: 100, converted: 130 }], 500, "USD"),
    ).toBe("£ 100.00 of it is in GBP: a 10% move in the pound is ± $ 13 on the bill.");
    expect(
      foreignText(
        [
          { code: "EUR", amount: 100, converted: 110 },
          { code: "GBP", amount: 100, converted: 130 },
        ],
        500,
        "USD",
      ),
    ).toBe("€ 100.00 and £ 100.00 of it are foreign: a 10% move in the rates is ± $ 24 on the bill.");
    expect(foreignText([{ code: "CHF", amount: 100, converted: null }], 500, "USD")).toBe(
      "CHF 100.00 of it is in CHF, which has no price in the ledger.",
    );
    expect(foreignText([], 500, "USD")).toBeNull();
  });
});

describe("beatenText", () => {
  const loan = {
    account: "Liabilities:Loan:Student",
    label: "Student Loan",
    peak: 6240,
    principal_paid: 6240,
    interest_paid: 278.5,
    first: "2024-01-01",
    last: "2025-11-05",
  };

  test("is the receipt for a debt that reached zero", () => {
    expect(beatenText(loan, "USD")).toBe(
      "$ 6,240 at its peak, paid off in 1 yr 10 mo, Jan 2024 to Nov 2025, for $ 278.50 in interest: 4.5% of the debt.",
    );
    expect(beatenText({ ...loan, interest_paid: 0 }, "USD")).toBe(
      "$ 6,240 at its peak, paid off in 1 yr 10 mo, Jan 2024 to Nov 2025, and not a cent in interest.",
    );
    expect(beatenText({ ...loan, first: "2025-11-05" }, "USD")).toBe(
      "$ 6,240 at its peak, paid off within the month, in Nov 2025, for $ 278.50 in interest: 4.5% of the debt.",
    );
  });
});

describe("the masthead's tooltips", () => {
  test("coverLine is whether the cash covers the cards", () => {
    expect(coverLine({ cash: 143637.29, owed: 3229.05, covered: true, after: 140408.24 }, "USD")).toBe(
      "The cards are covered 44 times over: $ 143,637 in budget accounts against $ 3,229 on them.",
    );
    expect(coverLine({ cash: 4000, owed: 3229.05, covered: true, after: 770.95 }, "USD")).toBe(
      "The cards are covered, with $ 771 to spare: $ 4,000 in budget accounts against $ 3,229 on them.",
    );
    expect(coverLine({ cash: 1000, owed: 3229.05, covered: false, after: -2229.05 }, "USD")).toBe(
      "The cards are not covered: $ 1,000 in budget accounts against $ 3,229 on them, $ 2,229 short.",
    );
    expect(coverLine({ cash: 1000, owed: 0, covered: true, after: 1000 }, "USD")).toBeNull();
  });

  test("earnedLine puts interest paid beside interest earned", () => {
    const window: [string, string] = ["2025-09", "2026-08"];
    expect(earnedLine({ year: 709.24, earned_year: 2473.68, window }, "USD")).toBe(
      "Over Sep – Aug 2026, $ 709 went out in interest and $ 2,474 came in from savings: a net gain of $ 1,764.",
    );
    expect(earnedLine({ year: 709.24, earned_year: 100, window }, "USD")).toBe(
      "Over Sep – Aug 2026, $ 709 went out in interest and $ 100 came in from savings: a net cost of $ 609.",
    );
    expect(earnedLine({ year: 0, earned_year: 0, window: null }, "USD")).toBeNull();
  });
});

describe("emphasize", () => {
  test("picks the amounts, shares, months and spans out of a sentence", () => {
    expect(
      emphasize("Costs $ 81 a month, 9.5% blended; gone by Sep 2027, 1 yr 2 mo away, at BRL 1,000."),
    ).toEqual([
      { text: "Costs ", strong: false },
      { text: "$ 81", strong: true },
      { text: " a month, ", strong: false },
      { text: "9.5%", strong: true },
      { text: " blended; gone by ", strong: false },
      { text: "Sep 2027", strong: true },
      { text: ", ", strong: false },
      { text: "1 yr 2 mo", strong: true },
      { text: " away, at ", strong: false },
      { text: "BRL 1,000", strong: true },
      { text: ".", strong: false },
    ]);
  });

  test("leaves a span alone when the unit is the start of a longer word", () => {
    expect(emphasize("Over the last 3 months, 1 yr 2 months ago.")).toEqual([
      { text: "Over the last 3 months, ", strong: false },
      { text: "1 yr", strong: true },
      { text: " 2 months ago.", strong: false },
    ]);
  });
});

describe("costShares", () => {
  test("puts each debt's share of the balance against its share of the cost", () => {
    const shares = costShares([
      debt({ account: "car", label: "Car", owed: 5000, rate: 0.05 }),
      debt({ account: "store", label: "Store", owed: 2500, rate: 0.24 }),
      debt({ account: "sofa", label: "Sofa", owed: 2500, rate: 0 }),
      debt({ account: "done", label: "Done", owed: 0, rate: 0.2 }),
    ]);
    expect(shares.map((s) => s.account)).toEqual(["car", "store", "sofa"]);
    expect(shares.map((s) => s.owed)).toEqual([0.5, 0.25, 0.25]);
    expect(shares.map((s) => s.cost.toFixed(3))).toEqual(["0.294", "0.706", "0.000"]);
  });

  test("has no cost to share when nothing charges interest", () => {
    const shares = costShares([
      debt({ account: "a", owed: 100, rate: 0 }),
      debt({ account: "b", owed: 300, rate: null }),
    ]);
    expect(shares.map((s) => s.cost)).toEqual([0, 0]);
    expect(shares.map((s) => s.owed)).toEqual([0.25, 0.75]);
  });
});

describe("hues", () => {
  test("gives every owing debt a colour in order, cycling after five", () => {
    const debts = ["a", "b", "c", "d", "e", "f"].map((account) =>
      debt({ account, owed: 10 }),
    );
    const h = hues([...debts, debt({ account: "paid", owed: 0 })]);
    expect([...h.entries()]).toEqual([
      ["a", "d1"],
      ["b", "d2"],
      ["c", "d3"],
      ["d", "d4"],
      ["e", "d5"],
      ["f", "d1"],
    ]);
  });
});

describe("rateTone", () => {
  test("reads a rate against the return the ledger assumes", () => {
    expect(rateTone(null, 0.05)).toBeNull();
    expect(rateTone(0, 0.05)).toBe("free");
    expect(rateTone(0.03, 0.05)).toBe("cheap");
    expect(rateTone(0.0528, 0.05)).toBe("dear");
    expect(rateTone(0.2388, 0.05)).toBe("steep");
  });
});

describe("stripMarks", () => {
  const up = (date: string, label = date, amount = 1) => ({
    account: label,
    label,
    date,
    amount,
  });

  test("places each payment along the next month and staggers close neighbours", () => {
    const marks = stripMarks(
      [up("2026-09-27"), up("2026-09-10"), up("2026-09-15"), up("2026-09-18")],
      "2026-09-02",
    );
    expect(marks.map((m) => m.date)).toEqual([
      "2026-09-10",
      "2026-09-15",
      "2026-09-18",
      "2026-09-27",
    ]);
    expect(marks.map((m) => m.x.toFixed(3))).toEqual(["0.258", "0.419", "0.516", "0.806"]);
    expect(marks.map((m) => m.lane)).toEqual([0, 0, 1, 0]);
  });

  test("stacks payments on the same day, and keeps the strip's ends", () => {
    const marks = stripMarks(
      [up("2026-09-02", "a"), up("2026-09-02", "b"), up("2026-09-02", "c"), up("2026-10-03", "d")],
      "2026-09-02",
    );
    expect(marks.map((m) => m.lane)).toEqual([0, 1, 2, 0]);
    expect(marks.map((m) => m.x)).toEqual([0, 0, 0, 1]);
  });
});

describe("addDays", () => {
  test("steps over month and year ends", () => {
    expect(addDays("2026-09-02", 7)).toBe("2026-09-09");
    expect(addDays("2026-09-28", 5)).toBe("2026-10-03");
    expect(addDays("2026-12-30", 3)).toBe("2027-01-02");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
  });
});

describe("monthPlan", () => {
  const today = "2026-06-10";
  const carried = { charges: 250, payments: 0, carried: 355.21, in_full: false };
  const bike = debt({
    account: "B",
    label: "Bike",
    owed: 600,
    rate: 0.06,
    payment: 60,
    next_due: "2026-06-12",
  });
  const car = debt({
    account: "C",
    label: "Car",
    owed: 9000,
    rate: 0.03,
    payment: 400,
    next_due: "2026-06-15",
    payoff: { months: 24, month: "2028-06", interest: 280.5 },
  });
  const store = debt({
    account: "S",
    label: "Store",
    kind: "revolving",
    owed: 783.68,
    rate: 0.24,
    payment: 300,
    next_due: "2026-06-20",
    payoff: { months: 3, month: "2026-09", interest: 25.11 },
    cycle: carried,
  });
  const everyday = debt({
    account: "E",
    label: "Everyday",
    kind: "revolving",
    owed: 420,
    payment: 380,
    next_due: "2026-06-25",
    cycle: cleared,
  });
  const student = debt({
    account: "T",
    label: "Student",
    owed: 5000,
    rate: 0.07,
    payment: 200,
    next_due: "2026-07-05",
  });
  const debts = [car, student, store, everyday, bike];
  const upcoming = [
    { account: "B", label: "Bike", date: "2026-06-12", amount: 60 },
    { account: "C", label: "Car", date: "2026-06-15", amount: 400 },
    { account: "S", label: "Store", date: "2026-06-20", amount: 300 },
    { account: "E", label: "Everyday", date: "2026-06-25", amount: 420 },
    { account: "T", label: "Student", date: "2026-07-05", amount: 200 },
  ];

  test("is what goes out anyway, by the day it goes", () => {
    expect(monthPlan(debts, upcoming, 0, "avalanche", 0.05, today)).toEqual({
      left: 0,
      rows: [
        { account: "B", label: "Bike", date: "2026-06-12", usual: 60, extra: 0, after: 540 },
        { account: "C", label: "Car", date: "2026-06-15", usual: 400, extra: 0, after: 8600 },
        { account: "S", label: "Store", date: "2026-06-20", usual: 300, extra: 0, after: 483.68 },
        { account: "E", label: "Everyday", date: "2026-06-25", usual: 420, extra: 0, after: 0 },
        { account: "T", label: "Student", date: "2026-07-05", usual: 200, extra: 0, after: 4800 },
      ],
    });
  });

  test("sends the spare down the attack order, past the loans cheaper than investing", () => {
    // Dearest first: the Store at 24%, then the Student at 7%. The Bike
    // at 6% is next in line but the money runs out; the Car at 3%
    // costs less than a portfolio makes, so it is never in line.
    const a = monthPlan(debts, upcoming, 1000, "avalanche", 0.05, today);
    expect(a.rows.map((r) => [r.account, r.extra, r.after])).toEqual([
      ["B", 0, 540],
      ["C", 0, 8600],
      ["S", 483.68, 0],
      ["E", 0, 0],
      ["T", 516.32, 4283.68],
    ]);
    expect(a.left).toBe(0);
    // Smallest first: the Bike, then the Store; the Car is skipped for
    // the same reason, and the Everyday card is spending, not debt.
    const s = monthPlan(debts, upcoming, 1000, "snowball", 0.05, today);
    expect(s.rows.map((r) => [r.account, r.extra, r.after])).toEqual([
      ["B", 540, 0],
      ["C", 0, 8600],
      ["S", 460, 23.68],
      ["E", 0, 0],
      ["T", 0, 4800],
    ]);
  });

  test("a debt whose payment already went out gets its extra today", () => {
    expect(monthPlan([store], [], 100, "avalanche", 0.05, today)).toEqual({
      left: 0,
      rows: [
        { account: "S", label: "Store", date: today, usual: 0, extra: 100, after: 683.68 },
      ],
    });
  });

  test("more than everything owed leaves the rest unplaced", () => {
    const p = monthPlan([store], upcoming.slice(2, 3), 5000, "avalanche", 0.05, today);
    expect(p.rows[0]?.extra).toBe(483.68);
    expect(p.rows[0]?.after).toBe(0);
    expect(p.left).toBe(4516.32);
  });

  test("dueEvents is one calendar event per payment coming up", () => {
    expect(dueEvents(upcoming, debts, "USD")).toEqual([
      {
        uid: "B@you-need-a-bean",
        date: "2026-06-12",
        summary: "Bike: $ 60 due",
        description: "The usual payment on the loan; $ 540.00 left after it.",
        day: 12,
        count: 12,
      },
      {
        uid: "C@you-need-a-bean",
        date: "2026-06-15",
        summary: "Car: $ 400 due",
        description: "The usual payment on the loan; $ 8,600.00 left after it.",
        day: 15,
        count: 24,
      },
      {
        uid: "S@you-need-a-bean",
        date: "2026-06-20",
        summary: "Store: $ 300 due",
        description: "The minimum on the card; $ 483.68 left after it.",
        day: 20,
        count: 3,
      },
      {
        uid: "E@you-need-a-bean",
        date: "2026-06-25",
        summary: "Everyday: statement due",
        description: "Whatever is on the card by then; $ 420.00 this time.",
        day: 25,
        count: 12,
      },
      {
        uid: "T@you-need-a-bean",
        date: "2026-07-05",
        summary: "Student: $ 200 due",
        description: "The usual payment on the loan; $ 4,800.00 left after it.",
        day: 5,
        count: 12,
      },
    ]);
  });
});

describe("sliderStart", () => {
  test("sliderStart snaps the usual surplus onto the slider", () => {
    expect(sliderStart(2029, { max: 2500, step: 100 })).toBe(2000);
    expect(sliderStart(0, { max: 500, step: 20 })).toBe(0);
    expect(sliderStart(9000, { max: 2500, step: 100 })).toBe(2500);
    expect(sliderStart(31, { max: 500, step: 20 })).toBe(40);
  });
});

describe("treadmillFacts", () => {
  const base = {
    label: "Store Card",
    kind: "revolving" as const,
    owed: 783.68,
    rate: 0.24,
    payment: 300,
    payoff: { months: 3, month: "2026-09", interest: 25.11 },
  };
  const walk = {
    months: [],
    pace: 3,
    charged: 700,
    paid: 900,
    net: 66.67,
    payoff: { months: 14, month: "2027-08", interest: 118.5 },
  };

  test("reads the net, where that pace really leads, and the interest on the way", () => {
    expect(treadmillFacts(debt({ ...base, treadmill: walk }), "USD")).toEqual([
      { lbl: "Net a month", v: "+$ 67", hint: "$ 900 paid · $ 700 charged", tone: "ok" },
      { lbl: "At that pace", v: "Aug 2027", hint: "not the Sep 2026 the minimum says" },
      { lbl: "Interest on the way", v: "$ 119", hint: "$ 25 at the minimum alone", tone: "bad" },
    ]);
  });

  test("a card growing, or standing still, never gets there", () => {
    expect(
      treadmillFacts(
        debt({ ...base, treadmill: { ...walk, charged: 300, paid: 60, net: -80, payoff: null } }),
        "USD",
      ),
    ).toEqual([
      { lbl: "Net a month", v: "−$ 80", hint: "$ 60 paid · $ 300 charged", tone: "bad" },
      { lbl: "At that pace", v: "never", hint: "the payments trail the charges", tone: "bad" },
    ]);
    expect(
      treadmillFacts(
        debt({ ...base, treadmill: { ...walk, charged: 300, paid: 300, net: 0, payoff: null } }),
        "USD",
      ),
    ).toEqual([
      { lbl: "Net a month", v: "$ 0", hint: "$ 300 paid · $ 300 charged" },
      { lbl: "At that pace", v: "never", hint: "the payments only cover the charges", tone: "bad" },
    ]);
  });

  test("a card shrinking too slowly for its interest never clears either", () => {
    expect(
      treadmillFacts(
        debt({ ...base, treadmill: { ...walk, charged: 885, paid: 900, net: 5, payoff: null } }),
        "USD",
      ),
    ).toEqual([
      { lbl: "Net a month", v: "+$ 5", hint: "$ 900 paid · $ 885 charged", tone: "ok" },
      { lbl: "At that pace", v: "never", hint: "the interest outruns it", tone: "bad" },
    ]);
  });

  test("a pace that agrees with the minimum says so", () => {
    const quiet = {
      ...walk,
      pace: 1,
      charged: 0,
      paid: 300,
      net: 300,
      payoff: { months: 3, month: "2026-09", interest: 25.11 },
    };
    expect(treadmillFacts(debt({ ...base, treadmill: quiet }), "USD")).toEqual([
      { lbl: "Net a month", v: "+$ 300", hint: "$ 300 paid · nothing charged", tone: "ok" },
      { lbl: "At that pace", v: "Sep 2026", hint: "as the minimum says" },
      { lbl: "Interest on the way", v: "$ 25", hint: "as the minimum says", tone: "bad" },
    ]);
    expect(
      treadmillFacts(debt({ ...base, payoff: null, treadmill: walk }), "USD"),
    ).toEqual([
      { lbl: "Net a month", v: "+$ 67", hint: "$ 900 paid · $ 700 charged", tone: "ok" },
      { lbl: "At that pace", v: "Aug 2027", hint: "the minimum alone never gets there" },
      { lbl: "Interest on the way", v: "$ 119", hint: "at that pace", tone: "bad" },
    ]);
  });

  test("has nothing to say about a card that is not on the treadmill", () => {
    expect(treadmillFacts(debt(base), "USD")).toBeNull();
  });
});

describe("verdict", () => {
  test("puts a loan's rate against the return a portfolio is assumed to make, in a few words", () => {
    expect(verdict(0.0528, 0.05)).toEqual({
      call: "Clear it first",
      why: "5.3% guaranteed beats the 5% a portfolio is assumed to make",
      tone: "go",
    });
    expect(verdict(0.03, 0.05)).toEqual({
      call: "Invest instead",
      why: "3% is under the 5% a portfolio is assumed to make",
      tone: "hold",
    });
    expect(verdict(0, 0.05)).toEqual({
      call: "No hurry",
      why: "at 0% the same money earns more in savings",
      tone: "free",
    });
    expect(verdict(null, 0.05)).toBeNull();
  });
});

describe("columns", () => {
  const same = () => 1;

  test("stands the biggest two at the top of each column", () => {
    expect(columns(["a", "b", "c"], same)).toEqual([
      ["a", "c"],
      ["b"],
    ]);
    expect(columns(["a", "b", "c", "d"], same)).toEqual([
      ["a", "c"],
      ["b", "d"],
    ]);
  });

  test("sends each card to whichever column is shorter", () => {
    // One tall card is worth the other three, so they stack beside it
    // instead of dealing out every other one and leaving a gap.
    const tall: Record<string, number> = { a: 9, b: 3, c: 3, d: 3 };
    expect(columns(["a", "b", "c", "d"], (k) => tall[k]!)).toEqual([
      ["a"],
      ["b", "c", "d"],
    ]);
  });

  test("one debt has nothing to spread, and none has nothing to lay out", () => {
    expect(columns(["a"], same)).toEqual([["a"]]);
    expect(columns([], same)).toEqual([]);
  });
});
