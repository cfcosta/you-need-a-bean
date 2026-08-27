import { describe, expect, test } from "bun:test";

import type { Posting } from "./api";
import { counterpart, prevMonth } from "./register";

const p = (account: string, amount: number | null = null): Posting => ({
  account,
  amount,
  currency: amount == null ? null : "USD",
});

describe("counterpart", () => {
  test("names the other side of a two-legged transaction", () => {
    const r = counterpart(
      [p("Assets:Cash", -50), p("Expenses:Food:Groceries", 50)],
      "Assets:Cash",
    );
    expect(r).toEqual({
      accounts: ["Expenses:Food:Groceries"],
      label: "Food:Groceries",
      kind: "expense",
    });
  });

  // The root says what kind of account it is, and the row already
  // says that in colour. Spelling it out again costs the column the
  // width it needs for the part that differs.
  test("drops the root the row's tone already carries", () => {
    const salary = counterpart(
      [p("Assets:Cash"), p("Income:Salary", -1000)],
      "Assets:Cash",
    );
    expect(salary.label).toBe("Salary");
    expect(salary.kind).toBe("income");

    const moved = counterpart(
      [p("Assets:Cash"), p("Assets:US:BofA:Checking", -20)],
      "Assets:Cash",
    );
    expect(moved.label).toBe("US:BofA:Checking");
    expect(moved.kind).toBe("transfer");
  });

  test("counts the legs it cannot name in one breath", () => {
    const r = counterpart(
      [
        p("Assets:Cash", -90),
        p("Expenses:Food:Groceries", 50),
        p("Expenses:Fun:Games", 40),
      ],
      "Assets:Cash",
    );
    expect(r.label).toBe("2 accounts");
    expect(r.kind).toBe("expense");
  });

  test("a transaction that is several things at once is none of them", () => {
    const r = counterpart(
      [p("Assets:Cash", 900), p("Income:Salary", -1000), p("Expenses:Tax", 100)],
      "Assets:Cash",
    );
    expect(r.kind).toBe("mixed");
  });

  // A multi-currency leg arrives as one posting per amount, which is
  // one account said twice, not two accounts.
  test("counts an account once however many legs it has", () => {
    const r = counterpart(
      [p("Assets:Cash", -50), p("Expenses:Food", 30), p("Expenses:Food", 20)],
      "Assets:Cash",
    );
    expect(r.accounts).toEqual(["Expenses:Food"]);
    expect(r.label).toBe("Food");
  });

  test("has nobody to name when the account is the whole story", () => {
    expect(counterpart([p("Assets:Cash", 10)], "Assets:Cash")).toEqual({
      accounts: [],
      label: "—",
      kind: "none",
    });
  });
});

describe("prevMonth", () => {
  test("steps back one month", () => {
    expect(prevMonth("2026-08")).toBe("2026-07");
  });

  test("steps back over the turn of the year", () => {
    expect(prevMonth("2026-01")).toBe("2025-12");
  });
});
