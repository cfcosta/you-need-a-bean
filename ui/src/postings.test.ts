import { describe, expect, test } from "bun:test";

import type { Posting } from "./api";
import { postingFlow } from "./postings";

const p = (
  account: string,
  amount: number | null,
  currency: string | null = "USD",
): Posting => ({ account, amount, currency });

describe("postingFlow", () => {
  test("reads a plain payment as money leaving one account for another", () => {
    const flow = postingFlow([
      p("Expenses:Utilities:Power", 120),
      p("Assets:Bank:Checking", -120),
    ]);
    expect(flow.note).toBeNull();
    // Where it came from first, then where it went — the way a flow reads.
    expect(flow.rows.map((r) => [r.account, r.side, r.share])).toEqual([
      ["Assets:Bank:Checking", "from", 1],
      ["Expenses:Utilities:Power", "to", 1],
    ]);
    // Two legs of the same size have no proportion worth drawing.
    expect(flow.split).toBe(false);
  });

  test("splits the account into a muted path and its leaf", () => {
    const [row] = postingFlow([p("Assets:Bank:Checking", -10)]).rows;
    expect(row?.path).toBe("Assets:Bank:");
    expect(row?.leaf).toBe("Checking");

    const [bare] = postingFlow([p("Equity", 10)]).rows;
    expect(bare?.path).toBe("");
    expect(bare?.leaf).toBe("Equity");
  });

  test("hangs a split bill off its source, biggest part first", () => {
    const flow = postingFlow([
      p("Expenses:Event:Venue", 700),
      p("Expenses:Event:Catering", 200),
      p("Expenses:Event:Supplies", 100),
      p("Assets:Bank:Checking", -1000),
    ]);
    expect(flow.rows.map((r) => r.leaf)).toEqual([
      "Checking",
      "Venue",
      "Catering",
      "Supplies",
    ]);
    // Shares are of the biggest posting, so the source spans the row and
    // the parts read against it.
    expect(flow.rows.map((r) => Math.round(r.share * 100))).toEqual([
      100, 70, 20, 10,
    ]);
    expect(flow.split).toBe(true);
    expect(flow.note).toBeNull();
  });

  test("reads a refund the other way round", () => {
    const flow = postingFlow([
      p("Expenses:Food:Groceries", -20),
      p("Assets:Cash", 20),
    ]);
    expect(flow.rows.map((r) => [r.leaf, r.side])).toEqual([
      ["Groceries", "from"],
      ["Cash", "to"],
    ]);
  });

  test("calls it a split when either side has more than one leg", () => {
    const paidTwoWays = postingFlow([
      p("Expenses:Fun:Games", 100),
      p("Assets:Cash", -40),
      p("Liabilities:Card", -60),
    ]);
    expect(paidTwoWays.rows.map((r) => r.leaf)).toEqual([
      "Card",
      "Cash",
      "Games",
    ]);
    expect(paidTwoWays.split).toBe(true);
  });

  test("says so when the postings do not add up", () => {
    expect(
      postingFlow([
        p("Expenses:Fun:Games", 50),
        p("Assets:Cash", -40),
      ]).note,
    ).toBe("does not balance");

    expect(
      postingFlow([
        p("Assets:Broker:DEMO", 10, "DEMO"),
        p("Assets:Cash", -500, "USD"),
      ]).note,
    ).toBe("2 currencies");

    const elided = postingFlow([
      p("Expenses:Fun:Games", 50),
      p("Assets:Cash", null, null),
    ]);
    expect(elided.note).toBe("one amount left to beancount");
    // Nothing is known about it, so it gets no bar and sits last.
    expect(elided.rows[1]?.side).toBe("auto");
    expect(elided.rows[1]?.share).toBe(0);
  });

  test("keeps cent-level rounding from crying wolf", () => {
    expect(
      postingFlow([
        p("Expenses:Fun:Games", 33.33),
        p("Expenses:Fun:Toys", 33.33),
        p("Expenses:Fun:Books", 33.34),
        p("Assets:Cash", -100),
      ]).note,
    ).toBeNull();
  });

  test("has nothing to say about no postings", () => {
    expect(postingFlow([])).toEqual({ rows: [], note: null, split: false });
  });
});
