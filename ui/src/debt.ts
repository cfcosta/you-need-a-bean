/**
 * The arithmetic behind the liabilities page that the server does not
 * do for it: the payoff at a payment the reader is only trying out,
 * and the small spellings the cards share.
 */

import type { DebtNotice } from "./api";
import { fmt, monthShort } from "./format";

/** How far ahead a payoff is followed before it is called never. The
 * server stops at the same place, so the two agree on what a debt
 * with no end looks like. */
const MAX_MONTHS = 1200;

/** To the cent, a half going to the even side, which is how the
 * server rounds. */
const cents = (v: number) => {
  const s = v * 100;
  const f = Math.floor(s);
  const frac = s - f;
  if (Math.abs(frac - 0.5) < 1e-7) return (f % 2 === 0 ? f : f + 1) / 100;
  return Math.round(s) / 100;
};

export interface Amortization {
  months: number;
  /** Interest paid over those months. */
  interest: number;
  /** The balance left after each payment, ending on zero. */
  curve: number[];
}

/** Pay `payment` a month against `owed` at a yearly `rate`, the way the
 * server does: interest accrues monthly to the cent, then the payment
 * comes off. Null when the payment never gets there. */
export function amortize(
  owed: number,
  rate: number,
  payment: number,
): Amortization | null {
  if (owed <= 0) return { months: 0, interest: 0, curve: [] };
  if (payment <= 0) return null;
  const monthly = rate / 12;
  let balance = owed;
  let interest = 0;
  const curve: number[] = [];
  for (let m = 1; m <= MAX_MONTHS; m++) {
    const accrued = cents(balance * monthly);
    if (payment <= accrued) return null;
    interest = cents(interest + accrued);
    balance = cents(balance + accrued - payment);
    curve.push(Math.max(0, balance));
    if (balance <= 0) return { months: m, interest, curve };
  }
  return null;
}

const NICE = [1, 2, 2.5, 5, 10];

/** The smallest round number at or above v: 1, 2, 2.5 or 5 times a
 * power of ten. */
function niceCeil(v: number): number {
  if (!(v > 0)) return 0;
  let mag = 10 ** Math.floor(Math.log10(v));
  // log10 is not exact at the powers of ten themselves.
  if (mag * 10 <= v) mag *= 10;
  if (mag > v) mag /= 10;
  for (const n of NICE) if (n * mag >= v - 1e-9) return n * mag;
  return 10 * mag;
}

/** How far the extra-payment slider runs: to about one more payment,
 * on a round number, in round steps — and never so short that a card's
 * small minimum leaves nothing to try. */
export function sliderRange(payment: number): { max: number; step: number } {
  const max = Math.max(50, niceCeil(payment));
  return { max, step: niceCeil(max / 25) };
}

/** 15 → "15th" */
export function ordinal(n: number): string {
  const r = n % 100;
  const suffix =
    r >= 11 && r <= 13 ? "th" : (["th", "st", "nd", "rd"][n % 10] ?? "th");
  return `${n}${suffix}`;
}

/** "2026-02-20" → "20 Feb" */
export function dayLabel(date: string): string {
  return `${Number(date.slice(8, 10))} ${monthShort(date)}`;
}

const utc = (d: string) =>
  Date.UTC(Number(d.slice(0, 4)), Number(d.slice(5, 7)) - 1, Number(d.slice(8, 10)));

/** Days from one date to another; negative when `to` comes first. */
export function daysBetween(from: string, to: string): number {
  return Math.round((utc(to) - utc(from)) / 86_400_000);
}

/** What a notice says. The server sends the fact; the sentence is ours. */
export function noticeText(n: DebtNotice, cur: string): string {
  switch (n.kind) {
    case "missed":
      return (
        `${n.label}: nothing paid yet this month, and it usually goes out ` +
        `on the ${ordinal(n.day ?? 0)}`
      );
    case "growing":
      return (
        `${n.label}: the balance carried from month to month has grown ` +
        `three months running`
      );
    case "overpaid":
      return (
        `${n.label} is ${fmt(n.amount ?? 0, cur)} in credit — a refund or ` +
        `a payment too many`
      );
  }
}
