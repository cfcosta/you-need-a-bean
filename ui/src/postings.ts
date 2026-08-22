/**
 * A transaction's postings as a flow: what the money went to, what it
 * came from, and how big each leg is against the biggest one. Pure
 * arithmetic over what the API sends — the drawing lives in the
 * inspector.
 */

import type { Posting } from "./api";

/** Which way the money moved. `auto` is beancount's elided amount. */
export type Side = "to" | "from" | "auto";

export interface FlowRow {
  account: string;
  /** Everything above the leaf, colons kept: "Expenses:Housing:". */
  path: string;
  leaf: string;
  amount: number | null;
  currency: string | null;
  /** Size against the biggest posting in the transaction, 0…1. */
  share: number;
  side: Side;
}

export interface Flow {
  rows: FlowRow[];
  /** Only set when the postings do not tell a clean, balanced,
   * single-currency story — a rare thing worth saying out loud. */
  note: string | null;
  /** One side of the transaction has several legs, so their sizes are
   * worth drawing. Two equal legs are just a payment. */
  split: boolean;
}

const RANK: Record<Side, number> = { from: 0, to: 1, auto: 2 };

export function postingFlow(postings: Posting[]): Flow {
  const amounts = postings.map((p) => p.amount).filter((a) => a != null);
  const max = Math.max(...amounts.map(Math.abs), 0);
  const rows: FlowRow[] = postings.map((p) => {
    const cut = p.account.lastIndexOf(":");
    return {
      account: p.account,
      path: cut < 0 ? "" : p.account.slice(0, cut + 1),
      leaf: p.account.slice(cut + 1),
      amount: p.amount,
      currency: p.currency,
      share: p.amount != null && max > 0 ? Math.abs(p.amount) / max : 0,
      side: p.amount == null ? "auto" : p.amount < 0 ? "from" : "to",
    };
  });
  rows.sort(
    (a, b) =>
      RANK[a.side] - RANK[b.side] ||
      b.share - a.share ||
      a.account.localeCompare(b.account),
  );

  const currencies = new Set(
    postings.map((p) => p.currency).filter((c) => c != null),
  );
  const sum = amounts.reduce((s, a) => s + a, 0);
  const note =
    currencies.size > 1
      ? `${currencies.size} currencies`
      : amounts.length < postings.length
        ? "one amount left to beancount"
        : // Cents can round; anything bigger is the ledger telling us
          // something, or a posting we failed to read.
          Math.abs(sum) >= 0.005
          ? "does not balance"
          : null;

  const legs = (side: Side) => rows.filter((r) => r.side === side).length;
  return { rows, note, split: legs("to") > 1 || legs("from") > 1 };
}
