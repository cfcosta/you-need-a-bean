/**
 * The arithmetic behind the liabilities page that the server does not
 * do for it: the payoff at a payment the reader is only trying out,
 * the order to attack the debts in, and the sentences the cards say.
 *
 * The server sends facts. Every sentence here is a reading of them,
 * so the page can say what a number means instead of labelling it.
 */

import type {
  Beaten,
  Collateral,
  Cover,
  Debt,
  DebtKind,
  DebtNotice,
  Foreign,
} from "./api";
import {
  duration,
  fmt,
  monthShort,
  monthYear,
  ratio,
  windowLabel,
} from "./format";
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

const attack = (debts: PlanDebt[], order: Order) =>
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

/** Which order to pay in, given both plans at the same extra. */
export function orderVerdict(
  avalanche: Plan | null,
  snowball: Plan | null,
  cur: string,
): string {
  if (avalanche == null && snowball == null) {
    return "Neither order gets there: the payments do not beat the interest.";
  }
  if (avalanche == null) return "Only smallest first gets there.";
  if (snowball == null) return "Only highest rate first gets there.";
  const same =
    avalanche.months === snowball.months &&
    avalanche.interest === snowball.interest &&
    avalanche.steps.every(
      (s, i) =>
        s.account === snowball.steps[i]?.account &&
        s.months === snowball.steps[i]?.months,
    );
  if (same) {
    return "Both orders come out the same here: nothing to choose between them.";
  }
  const saves = cents(snowball.interest - avalanche.interest);
  const sooner = snowball.months - avalanche.months;
  const parts: string[] = [];
  if (saves >= EPS) {
    parts.push(
      `Highest rate first saves ${fmt(saves, cur)}` +
        (sooner > 0
          ? ` and finishes ${duration(sooner)} sooner.`
          : sooner < 0
            ? `, though smallest first finishes ${duration(-sooner)} sooner.`
            : "; both orders finish the same month."),
    );
  } else if (saves <= -EPS) {
    parts.push(`Smallest first saves ${fmt(-saves, cur)} here, which is unusual.`);
  } else {
    parts.push("The two orders cost the same to within a cent.");
  }
  const first = snowball.steps[0];
  const later = avalanche.steps.find((s) => s.account === first?.account);
  if (first != null && later != null && later.months > first.months) {
    parts.push(
      `Smallest first clears the ${first.label} ${duration(later.months - first.months)} earlier, ` +
        "a quick win the numbers do not price.",
    );
  }
  return parts.join(" ");
}

/** Whether an extra dollar does more against this loan or in a
 * portfolio making `assumed` a year. */
export function investVerdict(rate: number | null, assumed: number): string | null {
  if (rate == null) return null;
  if (rate <= 1e-9) {
    return (
      "At 0% there is nothing to gain by paying this early: the same money " +
      "earns more in a savings account."
    );
  }
  if (rate >= assumed) {
    return (
      `Paying this down returns ${ratio(rate)} guaranteed, more than the ` +
      `${ratio(assumed)} a portfolio is assumed to make. Clear it before investing more.`
    );
  }
  return (
    `This costs ${ratio(rate)}, less than the ${ratio(assumed)} a portfolio is ` +
    "assumed to make. On paper the money does better invested, though a " +
    "paid-off loan has no bad years."
  );
}

/** What a loan is secured on, and whether selling it would clear the loan. */
export function securedText(c: Collateral, owed: number, cur: string): string {
  if (c.value == null || c.value <= EPS) {
    return `Secured by ${c.label}, which has no price in the ledger.`;
  }
  const share = ratio(owed / c.value);
  const gap = c.value - owed;
  return gap >= 0
    ? `Secured by ${c.label}, worth ${fmt(c.value, cur, 0)}. The loan is ${share} of that, ` +
        `so selling it would clear the loan with ${fmt(gap, cur, 0)} to spare.`
    : `Secured by ${c.label}, worth ${fmt(c.value, cur, 0)}. The loan is ${share} of that: ` +
        `selling it would leave ${fmt(-gap, cur, 0)} still owed.`;
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

/** What a loan's card says under its name: what goes out, how far
 * along it is, and where it ends. */
export function loanLede(d: Debt, cur: string): string {
  const parts: string[] = [];
  if (d.payment != null) {
    const last = d.payments[0];
    const when =
      d.due_day != null ? `on the ${ordinal(d.due_day)}` : "each month";
    const split =
      last == null
        ? ""
        : last.interest > EPS
          ? `, and last time ${fmt(last.interest, cur)} of it was interest`
          : ", all of it principal";
    parts.push(`${fmt(d.payment, cur, 0)} goes out ${when}${split}.`);
  } else {
    parts.push("No payment seen yet.");
  }
  if (d.progress != null) {
    const done =
      d.progress <= EPS
        ? "None"
        : d.progress >= 1 - EPS
          ? "All"
          : ratio(d.progress);
    parts.push(`${done} of the ${fmt(d.peak, cur, 0)} borrowed is paid off.`);
  }
  if (d.payoff != null) {
    const more =
      d.payoff.interest > EPS
        ? `with ${fmt(d.payoff.interest, cur, 0)} more in interest`
        : "with no interest to come";
    parts.push(`At this pace it is gone by ${monthYear(d.payoff.month)}, ${more}.`);
  } else if (d.payment != null && d.owed > EPS) {
    parts.push("At this payment it never ends: the interest outruns it.");
  }
  return parts.join(" ");
}

/** What a card's card says under its name. `from` is the first month
 * of the window the interest was counted over. */
export function cardLede(d: Debt, cur: string, from: string | null): string {
  const c = d.cycle;
  if (d.owed < -EPS) {
    return (
      `${fmt(-d.owed, cur)} in credit: a refund or a payment landed after the ` +
      "balance was cleared, and the next statement starts from there."
    );
  }
  if (c?.in_full === true) {
    if (d.owed <= EPS) return "Cleared in full last cycle, and nothing charged since.";
    const parts = [`${fmt(d.owed, cur)} charged since`];
    if (d.limit != null && d.utilisation != null) {
      parts.push(`${ratio(d.utilisation)} of the ${fmt(d.limit, cur, 0)} limit`);
    }
    if (d.next_due != null) parts.push(`due ${dayLabel(d.next_due)}`);
    return `Cleared in full last cycle. ${parts.join(", ")}.`;
  }
  if (c?.carried != null && c.carried > EPS) {
    const at = d.rate != null ? ` at ${ratio(d.rate)}` : "";
    const since = from != null ? ` since ${monthYear(from)}` : "";
    const cost =
      d.interest_paid > EPS
        ? `, which has cost ${fmt(d.interest_paid, cur, 0)} in interest${since}`
        : "";
    const head = `Carrying ${fmt(c.carried, cur)} from month to month${at}${cost}.`;
    if (d.payment == null) return head;
    const min = `At the ${fmt(d.payment, cur, 0)} minimum`;
    const tail =
      d.payoff != null
        ? `${min} it takes ${duration(d.payoff.months)}` +
          (d.payoff.interest > EPS
            ? ` and ${fmt(d.payoff.interest, cur, 0)} more.`
            : ".")
        : `${min} it never clears.`;
    return `${head} ${tail}`;
  }
  return `${fmt(d.owed, cur)} on the card.`;
}

/** What carrying everything costs. */
export function costLine(
  costYear: number,
  blended: number | null,
  cur: string,
): string {
  if (costYear <= EPS) return "Nothing here is charging interest.";
  const mix = blended != null ? `, ${ratio(blended)} blended` : "";
  return (
    `Carrying this costs about ${fmt(costYear / 12, cur, 0)} a month, ` +
    `${fmt(costYear, cur, 0)} a year at today's balances and rates${mix}.`
  );
}

const share = (v: number) => (v >= 0.9995 ? "all" : ratio(v));

/** Which debt is the biggest and which is the dearest, when those are
 * not the same one. */
export function shareLine(debts: Debt[], cur: string): string | null {
  void cur;
  const owing = debts.filter((d) => d.owed > EPS);
  if (owing.length < 2) return null;
  const total = owing.reduce((s, d) => s + d.owed, 0);
  const cost = (d: Debt) => d.owed * (d.rate ?? 0);
  const costs = owing.reduce((s, d) => s + cost(d), 0);
  const biggest = owing.reduce((a, b) => (b.owed > a.owed ? b : a));
  const done =
    biggest.progress != null ? `, and ${ratio(biggest.progress)} paid off` : "";
  if (costs <= EPS) {
    return (
      `The ${biggest.label} is the biggest, ${ratio(biggest.owed / total)} of what is owed, ` +
      "and none of it is charging interest."
    );
  }
  const dearest = owing.reduce((a, b) => (cost(b) > cost(a) ? b : a));
  if (dearest.account === biggest.account) {
    return (
      `The ${biggest.label} is both the biggest and the dearest: ` +
      `${ratio(biggest.owed / total)} of what is owed and ${share(cost(biggest) / costs)} of the cost.`
    );
  }
  return (
    `The ${dearest.label} is ${ratio(dearest.owed / total)} of what is owed but ` +
    `${share(cost(dearest) / costs)} of the cost; the ${biggest.label} is the biggest${done}.`
  );
}

/** How much an extra hundred a month is worth, in months off the end. */
const HUNDRED = 100;

/** When the last debt ends, and what would move that. `free` is the
 * server's date, at each debt's own payment kept up forever. */
export function freeLine(
  free: string | null,
  month: string,
  debts: Debt[],
  cur: string,
): string | null {
  const down = payingDown(debts);
  if (down.length === 0) return null;
  const base = plan(down, 0, "avalanche");
  const more = plan(down, HUNDRED, "avalanche");
  if (free == null) {
    const never =
      "At today's payments the last debt never clears: a payment is not beating its interest.";
    return more != null
      ? `${never} ${fmt(HUNDRED, cur, 0)} more a month would end it in ${duration(more.months)}.`
      : never;
  }
  const last = debts
    .filter((d) => down.some((p) => p.account === d.account) && d.payoff != null)
    .reduce<Debt | null>(
      (a, b) => (a == null || (b.payoff?.months ?? 0) > (a.payoff?.months ?? 0) ? b : a),
      null,
    );
  const who =
    last != null
      ? `, and that is the ${last.label}${last.kind === "revolving" ? " at its minimum" : ""}`
      : "";
  let text = `At today's payments the last debt clears in ${monthYear(free)}${who}.`;
  if (base != null) {
    const rolled = addMonths(month, base.months);
    const sooner = rolled < free;
    if (sooner) {
      text += ` Roll each payment into the next debt as one ends and it is ${monthYear(rolled)} instead`;
    }
    if (more != null && more.months < base.months) {
      text += sooner
        ? `; ${fmt(HUNDRED, cur, 0)} more a month on top brings that forward ${duration(base.months - more.months)}.`
        : ` ${fmt(HUNDRED, cur, 0)} more a month brings that forward ${duration(base.months - more.months)}.`;
    } else if (sooner) {
      text += ".";
    }
  }
  return text;
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

/** The things in a sentence a reader scans for: amounts, shares,
 * months and spans. */
const STRONG =
  /([−-]?(?:\$|€|£|¥|R\$|CA\$|A\$|MX\$|₹|₩|[A-Z]{3}) [\d,]+(?:\.\d+)?|\d+(?:\.\d+)?%|(?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec) \d{4}|\d+ yr(?: \d+ mo)?|\d+ mo)/g;

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
