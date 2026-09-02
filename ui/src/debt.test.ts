import { describe, expect, test } from "bun:test";

import type { Debt } from "./api";
import {
  amortize,
  beatenText,
  cardLede,
  costLine,
  coverLine,
  dayLabel,
  daysBetween,
  earnedLine,
  emphasize,
  foreignText,
  freeLine,
  investVerdict,
  loanLede,
  noticeText,
  orderVerdict,
  ordinal,
  payingDown,
  plan,
  securedText,
  shareLine,
  sliderRange,
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

describe("orderVerdict", () => {
  const debts = [
    { account: "A", label: "Sofa", kind: "installment" as const, owed: 300, rate: 0, payment: 50 },
    { account: "B", label: "Card", kind: "revolving" as const, owed: 1000, rate: 0.12, payment: 100 },
  ];

  test("weighs the interest saved against the quick win", () => {
    const a = plan(debts, 100, "avalanche");
    const s = plan(debts, 100, "snowball");
    expect(orderVerdict(a, s, "USD")).toBe(
      "Highest rate first saves $ 6.13; both orders finish the same month. " +
        "Smallest first clears the Sofa 4 mo earlier, a quick win the numbers do not price.",
    );
  });

  test("says when the orders agree", () => {
    const same = [
      { account: "A", label: "A", kind: "installment" as const, owed: 300, rate: 0.2, payment: 50 },
      { account: "B", label: "B", kind: "installment" as const, owed: 1000, rate: 0.05, payment: 100 },
    ];
    expect(orderVerdict(plan(same, 50, "avalanche"), plan(same, 50, "snowball"), "USD")).toBe(
      "Both orders come out the same here: nothing to choose between them.",
    );
  });

  test("says when neither gets there", () => {
    expect(orderVerdict(null, null, "USD")).toBe(
      "Neither order gets there: the payments do not beat the interest.",
    );
  });
});

describe("investVerdict", () => {
  test("puts a loan's rate against the return a portfolio is assumed to make", () => {
    expect(investVerdict(0.0528, 0.05)).toBe(
      "Paying this down returns 5.3% guaranteed, more than the 5% a portfolio is assumed to make. Clear it before investing more.",
    );
    expect(investVerdict(0.03, 0.05)).toBe(
      "This costs 3%, less than the 5% a portfolio is assumed to make. On paper the money does better invested, though a paid-off loan has no bad years.",
    );
    expect(investVerdict(0, 0.05)).toBe(
      "At 0% there is nothing to gain by paying this early: the same money earns more in a savings account.",
    );
    expect(investVerdict(null, 0.05)).toBeNull();
  });
});

describe("securedText", () => {
  const car = { account: "Assets:Vehicle:Car", label: "The Car", value: 14346.69 };

  test("says what the loan is against and whether that would clear it", () => {
    expect(securedText(car, 5234.5, "USD")).toBe(
      "Secured by The Car, worth $ 14,347. The loan is 36% of that, so selling it would clear the loan with $ 9,112 to spare.",
    );
    expect(securedText({ ...car, value: 4000.5 }, 5234.5, "USD")).toBe(
      "Secured by The Car, worth $ 4,001. The loan is 131% of that: selling it would leave $ 1,234 still owed.",
    );
    expect(securedText({ ...car, value: null }, 5234.5, "USD")).toBe(
      "Secured by The Car, which has no price in the ledger.",
    );
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

describe("loanLede", () => {
  const car = debt({
    label: "Car Loan",
    owed: 5234.5,
    peak: 18400,
    progress: 0.7155,
    rate: 0.0528,
    payment: 465,
    due_day: 18,
    payments: [{ date: "2026-08-18", principal: 440.03, interest: 24.97, total: 465 }],
    payoff: { months: 12, month: "2027-09", interest: 146.12 },
  });

  test("says what goes out, how far along it is, and where it ends", () => {
    expect(loanLede(car, "USD")).toBe(
      "$ 465 goes out on the 18th, and last time $ 24.97 of it was interest. " +
        "72% of the $ 18,400 borrowed is paid off. " +
        "At this pace it is gone by Sep 2027, with $ 146 more in interest.",
    );
  });

  test("an interest-free loan is just a countdown", () => {
    const sofa = debt({
      label: "Sofa",
      owed: 1725,
      peak: 1800,
      progress: 0.0417,
      rate: 0,
      payment: 75,
      due_day: 15,
      payments: [{ date: "2026-08-15", principal: 75, interest: 0, total: 75 }],
      payoff: { months: 23, month: "2028-08", interest: 0 },
    });
    expect(loanLede(sofa, "USD")).toBe(
      "$ 75 goes out on the 15th, all of it principal. " +
        "4.2% of the $ 1,800 borrowed is paid off. " +
        "At this pace it is gone by Aug 2028, with no interest to come.",
    );
  });

  test("a payment that never gets there says so", () => {
    expect(loanLede(debt({ ...car, payoff: null }), "USD")).toBe(
      "$ 465 goes out on the 18th, and last time $ 24.97 of it was interest. " +
        "72% of the $ 18,400 borrowed is paid off. " +
        "At this payment it never ends: the interest outruns it.",
    );
    expect(loanLede(debt({ label: "Loan", owed: 500, peak: 500, progress: 0 }), "USD")).toBe(
      "No payment seen yet. None of the $ 500 borrowed is paid off.",
    );
  });
});

describe("cardLede", () => {
  test("a card cleared in full is what has gone on it since", () => {
    const travel = debt({
      label: "Travel Card",
      kind: "revolving",
      owed: 827.67,
      limit: 6000,
      utilisation: 0.1379,
      due_day: 27,
      next_due: "2026-09-27",
      cycle: cleared,
    });
    expect(cardLede(travel, "USD", "2025-09")).toBe(
      "Cleared in full last cycle. $ 827.67 charged since, 14% of the $ 6,000 limit, due 27 Sep.",
    );
    expect(cardLede(debt({ ...travel, owed: 0, utilisation: 0, next_due: null }), "USD", null)).toBe(
      "Cleared in full last cycle, and nothing charged since.",
    );
  });

  test("a card in credit says where the credit came from", () => {
    const everyday = debt({ label: "Everyday", kind: "revolving", owed: -489, cycle: cleared });
    expect(cardLede(everyday, "USD", "2025-09")).toBe(
      "$ 489.00 in credit: a refund or a payment landed after the balance was cleared, and the next statement starts from there.",
    );
  });

  test("a card carrying a balance is what that costs and how long it lasts", () => {
    const store = debt({
      label: "Store Card",
      kind: "revolving",
      owed: 2890.38,
      rate: 0.2388,
      payment: 66.22,
      interest_paid: 335.83,
      cycle: { charges: 56.4, payments: 0, carried: 2283.09, in_full: false },
      payoff: { months: 103, month: "2035-04", interest: 3930.13 },
    });
    expect(cardLede(store, "USD", "2025-09")).toBe(
      "Carrying $ 2,283.09 from month to month at 24%, which has cost $ 336 in interest since Sep 2025. " +
        "At the $ 66 minimum it takes 8 yr 7 mo and $ 3,930 more.",
    );
    expect(cardLede(debt({ ...store, payoff: null }), "USD", null)).toBe(
      "Carrying $ 2,283.09 from month to month at 24%, which has cost $ 336 in interest. " +
        "At the $ 66 minimum it never clears.",
    );
  });

  test("a card the ledger cannot read closely is just its balance", () => {
    expect(cardLede(debt({ label: "Card", kind: "revolving", owed: 120 }), "USD", null)).toBe(
      "$ 120.00 on the card.",
    );
  });
});

describe("the masthead's sentences", () => {
  const store = debt({
    account: "S",
    label: "Store Card",
    kind: "revolving",
    owed: 2890.38,
    rate: 0.2388,
    payment: 66.22,
    cycle: { charges: 56.4, payments: 0, carried: 2283.09, in_full: false },
    payoff: { months: 103, month: "2035-04", interest: 3930.13 },
  });
  const car = debt({
    account: "C",
    label: "Car Loan",
    owed: 5234.5,
    progress: 0.7155,
    rate: 0.0528,
    payment: 465,
    payoff: { months: 12, month: "2027-09", interest: 146.12 },
  });
  const sofa = debt({
    account: "F",
    label: "Furniture",
    owed: 1725,
    progress: 0.0417,
    rate: 0,
    payment: 75,
    payoff: { months: 23, month: "2028-08", interest: 0 },
  });
  const travel = debt({ account: "T", label: "Travel", kind: "revolving", owed: 827.67, cycle: cleared });

  test("costLine is what carrying the debt costs", () => {
    expect(costLine(966.6, 0.0949, "USD")).toBe(
      "Carrying this costs about $ 81 a month, $ 967 a year at today's balances and rates, 9.5% blended.",
    );
    expect(costLine(0, null, "USD")).toBe("Nothing here is charging interest.");
  });

  test("shareLine is which debt is biggest and which is dearest", () => {
    expect(shareLine([car, store, sofa, travel], "USD")).toBe(
      "The Store Card is 27% of what is owed but 71% of the cost; the Car Loan is the biggest, and 72% paid off.",
    );
    expect(shareLine([car, sofa], "USD")).toBe(
      "The Car Loan is both the biggest and the dearest: 75% of what is owed and all of the cost.",
    );
    expect(shareLine([sofa, debt({ account: "G", label: "Gift", owed: 100 })], "USD")).toBe(
      "The Furniture is the biggest, 95% of what is owed, and none of it is charging interest.",
    );
    expect(shareLine([car], "USD")).toBeNull();
  });

  test("freeLine is when the last debt ends, and what would move it", () => {
    expect(freeLine("2035-04", "2026-09", [car, store, sofa, travel], "USD")).toBe(
      "At today's payments the last debt clears in Apr 2035, and that is the Store Card at its minimum. " +
        "Roll each payment into the next debt as one ends and it is Mar 2028 instead; " +
        "$ 100 more a month on top brings that forward 3 mo.",
    );
    expect(freeLine(null, "2026-09", [debt({ ...store, payoff: null })], "USD")).toBe(
      "At today's payments the last debt never clears: a payment is not beating its interest. " +
        "$ 100 more a month would end it in 1 yr 10 mo.",
    );
    expect(freeLine(null, "2026-09", [travel], "USD")).toBeNull();
  });

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
});
