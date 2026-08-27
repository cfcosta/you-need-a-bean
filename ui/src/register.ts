// Shaping a register row. Reading one is asking "and where did this
// money go?" over and over, so the answer has to fit in a glance.

import type { Posting } from "./api";

/** What kind of place the other side of a transaction is. */
export type Flow = "expense" | "income" | "transfer" | "mixed" | "none";

export interface Counterpart {
  /** The other accounts it touched, in the order it names them. */
  accounts: string[];
  /** One of them said short, or a count when there were several. */
  label: string;
  kind: Flow;
}

// Equity funds an account rather than earning it, and a root nobody
// recognises is not worth guessing at: both read as money moving.
const ROOTS: Record<string, Flow> = {
  Assets: "transfer",
  Liabilities: "transfer",
  Expenses: "expense",
  Income: "income",
};

/** An account without its root, which the row's colour already says. */
export function shortAccount(account: string): string {
  const [root, ...rest] = account.split(":");
  return rest.length > 0 ? rest.join(":") : (root ?? account);
}

/** The other side of `postings`, as seen from `account`. */
export function counterpart(
  postings: Posting[],
  account: string,
): Counterpart {
  const accounts: string[] = [];
  for (const p of postings) {
    if (p.account !== account && !accounts.includes(p.account)) {
      accounts.push(p.account);
    }
  }
  const first = accounts[0];
  if (first == null) return { accounts, label: "—", kind: "none" };
  const kinds = new Set(
    accounts.map((a) => ROOTS[a.split(":")[0] ?? ""] ?? "transfer"),
  );
  return {
    accounts,
    label:
      accounts.length === 1
        ? shortAccount(first)
        : `${accounts.length} accounts`,
    kind: kinds.size === 1 ? [...kinds][0]! : "mixed",
  };
}

/** The month before this one: "2026-01" → "2025-12". */
export function prevMonth(month: string): string {
  const n = Number(month.slice(0, 4)) * 12 + Number(month.slice(5, 7)) - 2;
  const year = Math.floor(n / 12);
  return `${year}-${String(n - year * 12 + 1).padStart(2, "0")}`;
}
