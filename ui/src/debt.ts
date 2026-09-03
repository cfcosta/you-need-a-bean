/**
 * The arithmetic behind the liabilities page that the server does not
 * do for it: the payoff at a payment the reader is only trying out,
 * the order to attack the debts in, and the readings the cards draw.
 *
 * The server sends facts. What is here is a reading of them: the page
 * shows what a number means with a figure, a mark or a call rather
 * than a paragraph.
 */

import type {
  Beaten,
  Cover,
  Debt,
  DebtKind,
  DebtNotice,
  Foreign,
  Upcoming,
} from "./api";
import {
  duration,
  fmt,
  monthShort,
  monthYear,
  ratio,
  windowLabel,
} from "./format";
import type { CalendarEvent } from "./ics";
import { addMonths } from "./months";

/** How far ahead a payoff is followed before it is called never. The
 * server stops at the same place, so the two agree on what a debt
 * with no end looks like. */
const MAX_MONTHS = 1200;

/** Below this a balance is the dust of a conversion, not a debt. */
const EPS = 0.005;

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

/** A debt as the planner sees it: what is owed, what it costs, and
 * what goes out against it each month. */
export interface PlanDebt {
  account: string;
  label: string;
  kind: DebtKind;
  owed: number;
  rate: number;
  payment: number;
}

/** The debts that are being paid down: loans with a balance, and cards
 * carrying one from month to month. A card cleared in full each cycle
 * is spending, not debt, and a card in credit owes nothing. */
export function payingDown(debts: Debt[]): PlanDebt[] {
  return debts.flatMap((d) =>
    d.owed > EPS &&
    (d.kind === "installment" || (d.cycle?.carried ?? 0) > EPS)
      ? [
          {
            account: d.account,
            label: d.label,
            kind: d.kind,
            owed: d.owed,
            rate: d.rate ?? 0,
            payment: d.payment ?? 0,
          },
        ]
      : [],
  );
}

/** Which debt the extra goes to first: the dearest, or the smallest. */
export type Order = "avalanche" | "snowball";

export interface PlanStep {
  account: string;
  label: string;
  /** Paid off after this many months. */
  months: number;
  /** Interest paid across every debt by then. */
  interest: number;
}

export interface Plan {
  order: Order;
  months: number;
  interest: number;
  /** In the order the debts end. */
  steps: PlanStep[];
}

/** The debts in the order the extra reaches them: dearest first, or
 * smallest first, the tie going to the other rule. */
export const attack = (debts: PlanDebt[], order: Order) =>
  [...debts].sort((a, b) =>
    order === "avalanche"
      ? b.rate - a.rate || a.owed - b.owed
      : a.owed - b.owed || b.rate - a.rate,
  );

/** Pay every debt its usual payment, put `extra` a month on the first
 * debt in `order`, and roll each payment into the next debt when one
 * ends. Null when the payments never get there. */
export function plan(debts: PlanDebt[], extra: number, order: Order): Plan | null {
  const ranked = attack(debts, order);
  const open = ranked.map((d) => ({ ...d, balance: d.owed }));
  const rank = (s: PlanStep) => ranked.findIndex((d) => d.account === s.account);
  const steps: PlanStep[] = [];
  let interest = 0;
  let rolled = 0;
  for (let m = 1; m <= MAX_MONTHS; m++) {
    if (open.length === 0) return { order, months: m - 1, interest, steps };
    let pool = cents(extra + rolled);
    for (const d of open) {
      const accrued = cents((d.balance * d.rate) / 12);
      d.balance = cents(d.balance + accrued);
      interest = cents(interest + accrued);
    }
    for (const d of open) {
      const pay = Math.min(d.payment, d.balance);
      d.balance = cents(d.balance - pay);
      pool = cents(pool + d.payment - pay);
    }
    for (const d of open) {
      if (pool <= 0) break;
      const pay = Math.min(pool, d.balance);
      d.balance = cents(d.balance - pay);
      pool = cents(pool - pay);
    }
    for (let i = open.length - 1; i >= 0; i--) {
      const d = open[i];
      if (d != null && d.balance <= 0) {
        rolled = cents(rolled + d.payment);
        open.splice(i, 1);
        steps.push({ account: d.account, label: d.label, months: m, interest });
      }
    }
    // Two debts ending the same month end in attack order, not in the
    // reverse the splice above walked them in.
    steps.sort((a, b) => a.months - b.months || rank(a) - rank(b));
    if (open.length === 0) return { order, months: m, interest, steps };
  }
  return null;
}

const NAMES: Record<string, string> = {
  USD: "dollar",
  EUR: "euro",
  GBP: "pound",
  JPY: "yen",
  BRL: "real",
  CHF: "franc",
  CAD: "Canadian dollar",
  AUD: "Australian dollar",
  MXN: "peso",
  INR: "rupee",
  KRW: "won",
};

/** What the part of a card held in another currency does to the bill
 * when the rate moves. */
export function foreignText(
  foreign: Foreign[],
  owed: number,
  cur: string,
): string | null {
  const held = foreign.filter((f) => f.amount > EPS);
  if (held.length === 0) return null;
  const priced = held.filter((f) => f.converted != null);
  const amounts = held.map((f) => fmt(f.amount, f.code));
  if (priced.length === 0) {
    const codes = held.map((f) => f.code).join(" and ");
    return `${amounts.join(" and ")} of it ${held.length > 1 ? "are" : "is"} in ${codes}, which ${held.length > 1 ? "have" : "has"} no price in the ledger.`;
  }
  const swing = priced.reduce((s, f) => s + (f.converted ?? 0), 0) * 0.1;
  const move = `a 10% move in ${held.length > 1 ? "the rates" : `the ${NAMES[held[0]?.code ?? ""] ?? "rate"}`}`;
  const total = priced.reduce((s, f) => s + (f.converted ?? 0), 0);
  const all = held.length === priced.length && total >= owed - 0.01;
  const what = all
    ? `All of it is ${amounts.join(" and ")}`
    : held.length > 1
      ? `${amounts.join(" and ")} of it are foreign`
      : `${amounts[0]} of it is in ${held[0]?.code}`;
  return `${what}: ${move} is ± ${fmt(swing, cur, 0)} on the bill.`;
}

const monthsBetween = (from: string, to: string) =>
  (Number(to.slice(0, 4)) - Number(from.slice(0, 4))) * 12 +
  Number(to.slice(5, 7)) -
  Number(from.slice(5, 7));

/** The receipt for a debt that reached zero. */
export function beatenText(b: Beaten, cur: string): string {
  const months = monthsBetween(b.first, b.last);
  const when =
    months <= 0
      ? `paid off within the month, in ${monthYear(b.last)}`
      : `paid off in ${duration(months)}, ${monthYear(b.first)} to ${monthYear(b.last)}`;
  const cost =
    b.interest_paid > EPS
      ? `for ${fmt(b.interest_paid, cur)} in interest: ${ratio(b.interest_paid / Math.max(b.peak, EPS))} of the debt`
      : "and not a cent in interest";
  return `${fmt(b.peak, cur, 0)} at its peak, ${when}, ${cost}.`;
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

/** Whether the cash on hand covers the cards. */
export function coverLine(cover: Cover, cur: string): string | null {
  if (cover.owed <= EPS) return null;
  const against = `${fmt(cover.cash, cur, 0)} in budget accounts against ${fmt(cover.owed, cur, 0)} on them`;
  if (!cover.covered) {
    return `The cards are not covered: ${against}, ${fmt(-cover.after, cur, 0)} short.`;
  }
  const times = cover.cash / cover.owed;
  return times >= 2
    ? `The cards are covered ${Math.floor(times)} times over: ${against}.`
    : `The cards are covered, with ${fmt(cover.after, cur, 0)} to spare: ${against}.`;
}

/** Interest paid beside interest earned, over the same window. */
export function earnedLine(
  i: { year: number; earned_year: number; window: [string, string] | null },
  cur: string,
): string | null {
  if (i.year <= EPS && i.earned_year <= EPS) return null;
  const net = i.earned_year - i.year;
  const verdict =
    net >= 0
      ? `a net gain of ${fmt(net, cur, 0)}`
      : `a net cost of ${fmt(-net, cur, 0)}`;
  return (
    `Over ${windowLabel(i.window)}, ${fmt(i.year, cur, 0)} went out in interest and ` +
    `${fmt(i.earned_year, cur, 0)} came in from savings: ${verdict}.`
  );
}

/** What the page calls on a loan: whether an extra dollar does more
 * against it or in a portfolio making `assumed` a year. A call in a
 * couple of words, and the reason in a few more. */
export interface Verdict {
  call: string;
  why: string;
  /** go: pay it down; hold: invest instead; free: nothing to gain. */
  tone: "go" | "hold" | "free";
}

export function verdict(rate: number | null, assumed: number): Verdict | null {
  if (rate == null) return null;
  if (rate <= 1e-9) {
    return {
      call: "No hurry",
      why: "at 0% the same money earns more in savings",
      tone: "free",
    };
  }
  const paper = `the ${ratio(assumed)} a portfolio is assumed to make`;
  return rate >= assumed
    ? { call: "Clear it first", why: `${ratio(rate)} guaranteed beats ${paper}`, tone: "go" }
    : { call: "Invest instead", why: `${ratio(rate)} is under ${paper}`, tone: "hold" };
}

/** The things in a sentence a reader scans for: amounts, shares,
 * months and spans. */
const STRONG =
  /([−-]?(?:\$|€|£|¥|R\$|CA\$|A\$|MX\$|₹|₩|[A-Z]{3}) [\d,]+(?:\.\d+)?|\d+(?:\.\d+)?%|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{4}|\d+ yr(?: \d+ mo)?\b|\d+ mo\b)/g;

export interface Segment {
  text: string;
  strong: boolean;
}

/** A sentence cut into the parts to set in bold and the parts not to. */
export function emphasize(text: string): Segment[] {
  return text
    .split(STRONG)
    .map((part, i) => ({ text: part, strong: i % 2 === 1 }))
    .filter((s) => s.text.length > 0);
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

const utc = (d: string) =>
  Date.UTC(Number(d.slice(0, 4)), Number(d.slice(5, 7)) - 1, Number(d.slice(8, 10)));

/** Days from one date to another; negative when `to` comes first. */
export function daysBetween(from: string, to: string): number {
  return Math.round((utc(to) - utc(from)) / 86_400_000);
}

/** The date `n` days on. */
export function addDays(date: string, n: number): string {
  return new Date(utc(date) + n * 86_400_000).toISOString().slice(0, 10);
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

/** One debt's share of what is owed against its share of what it all
 * costs, for drawing the two bars one above the other. */
export interface Share {
  account: string;
  label: string;
  owed: number;
  cost: number;
}

export function costShares(debts: Debt[]): Share[] {
  const owing = debts.filter((d) => d.owed > EPS);
  const total = owing.reduce((s, d) => s + d.owed, 0);
  const cost = (d: Debt) => d.owed * (d.rate ?? 0);
  const totalCost = owing.reduce((s, d) => s + cost(d), 0);
  return owing.map((d) => ({
    account: d.account,
    label: d.label,
    owed: total > 0 ? d.owed / total : 0,
    cost: totalCost > 0 ? cost(d) / totalCost : 0,
  }));
}

/** The colour class each owing debt keeps across the page, in the
 * order given, cycling after five. */
export function hues(debts: Debt[]): Map<string, string> {
  const out = new Map<string, string>();
  let k = 0;
  for (const d of debts) {
    if (d.owed <= EPS) continue;
    out.set(d.account, `d${(k % 5) + 1}`);
    k += 1;
  }
  return out;
}

/** How a rate reads against the return the ledger assumes a portfolio
 * makes: nothing, less than that, more, or a card's kind of rate. */
export type RateTone = "free" | "cheap" | "dear" | "steep";

export function rateTone(rate: number | null, assumed: number): RateTone | null {
  if (rate == null) return null;
  if (rate <= 1e-9) return "free";
  if (rate < assumed) return "cheap";
  if (rate < 0.15) return "dear";
  return "steep";
}

/** A payment placed along the next `days` days: `x` from 0 at today to
 * 1 at the end, and `lane` counting the earlier marks close enough to
 * overprint it, so the drawing can stagger their labels. */
export interface Mark {
  account: string;
  label: string;
  date: string;
  amount: number;
  x: number;
  lane: number;
}

export function stripMarks(
  upcoming: Upcoming[],
  today: string,
  days = 31,
  near = 0.12,
): Mark[] {
  const marks = upcoming
    .map((u) => ({
      ...u,
      x: Math.min(1, Math.max(0, daysBetween(today, u.date) / days)),
      lane: 0,
    }))
    .sort((a, b) => a.x - b.x || a.date.localeCompare(b.date));
  marks.forEach((m, i) => {
    m.lane = marks.slice(0, i).filter((p) => m.x - p.x < near).length;
  });
  return marks;
}

/** One line of this month's plan: what goes to a debt, and when. */
export interface PlanRow {
  account: string;
  label: string;
  date: string;
  /** What goes out anyway. */
  usual: number;
  /** What the spare adds on top. */
  extra: number;
  /** Left on the debt after both. */
  after: number;
}

export interface MonthPlan {
  /** By the day they go out. */
  rows: PlanRow[];
  /** The part of the spare with nowhere to go. */
  left: number;
}

/** The debts the spare goes to, in `order`: the ones being paid
 * down, less the loans cheaper than the return a portfolio is assumed
 * to make, where the same money does better invested. A card is
 * always in line: nothing invested beats a card's rate. */
export function targets(debts: Debt[], order: Order, assumed: number): PlanDebt[] {
  return attack(payingDown(debts), order).filter(
    (d) => d.kind === "revolving" || d.rate >= assumed - 1e-9,
  );
}

/** This month's payments: every one coming up at its usual amount,
 * and the spare `lump` placed on top down the attack order — as much
 * as each debt can take, on the day its payment goes, or today when
 * its payment has already gone. */
export function monthPlan(
  debts: Debt[],
  upcoming: Upcoming[],
  lump: number,
  order: Order,
  assumed: number,
  today: string,
): MonthPlan {
  const owed = new Map(debts.map((d) => [d.account, d.owed]));
  const rows: PlanRow[] = upcoming.map((u) => ({
    account: u.account,
    label: u.label,
    date: u.date,
    usual: u.amount,
    extra: 0,
    after: cents(Math.max(0, (owed.get(u.account) ?? 0) - u.amount)),
  }));
  let left = cents(Math.max(0, lump));
  for (const t of targets(debts, order, assumed)) {
    if (left <= 0) break;
    const row = rows.find((r) => r.account === t.account) ?? {
      account: t.account,
      label: t.label,
      date: today,
      usual: 0,
      extra: 0,
      after: cents(t.owed),
    };
    const pay = cents(Math.min(left, row.after));
    if (pay <= 0) continue;
    if (!rows.includes(row)) rows.push(row);
    row.extra = cents(row.extra + pay);
    row.after = cents(row.after - pay);
    left = cents(left - pay);
  }
  rows.sort(
    (a, b) => a.date.localeCompare(b.date) || a.label.localeCompare(b.label),
  );
  return { rows, left };
}

/** The payments coming up as calendar events, each repeating on its
 * day for as long as the debt has left: the payoff for a loan or a
 * carried card, a year for a card cleared every statement, whose
 * amount is whatever is on it by then. */
export function dueEvents(
  upcoming: Upcoming[],
  debts: Debt[],
  cur: string,
): CalendarEvent[] {
  return upcoming.map((u) => {
    const d = debts.find((x) => x.account === u.account);
    const loan = d?.kind !== "revolving";
    const paying = loan || (d?.cycle?.carried ?? 0) > EPS;
    const after = cents(Math.max(0, (d?.owed ?? 0) - u.amount));
    return {
      uid: `${u.account}@you-need-a-bean`,
      date: u.date,
      summary: paying
        ? `${u.label}: ${fmt(u.amount, cur, 0)} due`
        : `${u.label}: statement due`,
      description: paying
        ? `The ${loan ? "usual payment on the loan" : "minimum on the card"}; ` +
          `${fmt(after, cur)} left after it.`
        : `Whatever is on the card by then; ${fmt(u.amount, cur)} this time.`,
      day: Number(u.date.slice(8, 10)),
      count: paying && d?.payoff != null ? d.payoff.months : 12,
    };
  });
}

/** The usual month's surplus snapped onto the slider's steps, and
 * kept within its range. */
export function sliderStart(
  monthly: number,
  range: { max: number; step: number },
): number {
  return Math.max(
    0,
    Math.min(range.max, Math.round(monthly / range.step) * range.step),
  );
}

/** A figure with a label over it and a hint under it: what a card's
 * fact cell holds. */
export interface FactRead {
  lbl: string;
  v: string;
  hint: string;
  tone?: "ok" | "bad";
}

/** A carried card's treadmill read as figures: what it really moves
 * by a month, where that pace leads against where the minimum alone
 * says, and the interest on the way. Null off the treadmill. */
export function treadmillFacts(d: Debt, cur: string): FactRead[] | null {
  const t = d.treadmill;
  if (t == null) return null;
  const stated = d.payoff;
  const net: FactRead = {
    lbl: "Net a month",
    v: t.net > EPS ? `+${fmt(t.net, cur, 0)}` : fmt(t.net, cur, 0),
    hint: `${fmt(t.paid, cur, 0)} paid · ${t.charged > EPS ? `${fmt(t.charged, cur, 0)} charged` : "nothing charged"}`,
  };
  if (t.net > EPS) net.tone = "ok";
  else if (t.net < -EPS) net.tone = "bad";
  if (t.payoff == null) {
    return [
      net,
      {
        lbl: "At that pace",
        v: "never",
        hint:
          t.net < -EPS
            ? "the payments trail the charges"
            : t.net <= EPS
              ? "the payments only cover the charges"
              : "the interest outruns it",
        tone: "bad",
      },
    ];
  }
  const agrees = stated != null && stated.month === t.payoff.month;
  const interest: FactRead = {
    lbl: "Interest on the way",
    v: fmt(t.payoff.interest, cur, 0),
    hint:
      stated == null
        ? "at that pace"
        : Math.abs(stated.interest - t.payoff.interest) >= 1
          ? `${fmt(stated.interest, cur, 0)} at the minimum alone`
          : "as the minimum says",
  };
  if (t.payoff.interest > EPS) interest.tone = "bad";
  return [
    net,
    {
      lbl: "At that pace",
      v: monthYear(t.payoff.month),
      hint:
        stated == null
          ? "the minimum alone never gets there"
          : agrees
            ? "as the minimum says"
            : `not the ${monthYear(stated.month)} the minimum says`,
    },
    interest,
  ];
}
