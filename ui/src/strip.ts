/**
 * The month's headline numbers as lengths on one shared scale.
 *
 * Each tile used to carry a line of prose under it saying what its
 * number meant against another one — "25% of a typical month", "50% of
 * the month gone", "income − spending". A reader asking whether this
 * month is dear is comparing two quantities, so the strip draws them
 * to one scale and lets the comparison happen by eye.
 */

/** How far through a month we are. A `MonthView` satisfies it. */
export interface MonthPace {
  is_current: boolean;
  day: number;
  days_in_month: number;
}

/** Everything the geometry needs; a `MonthView` satisfies it. */
export interface StripInput extends MonthPace {
  income: number;
  spent: number;
  typical: number | null;
}

export interface StripBars {
  /** Each 0…1 of the biggest of the three, so the bars compare. */
  income: number;
  spent: number;
  /** Null when no earlier month had payments to take a median from. */
  typical: number | null;
  /** The gap between income and spending, as a length. */
  net: number;
  /** Whether that gap went the wrong way. */
  negative: boolean;
  /** How much of a still-running month has gone; null once it is over. */
  pace: number | null;
  /** Where an evenly-spent typical month would have you today, on the
   * same scale as the rest — so the distance from the fill to here is
   * how far ahead or behind you are. Null without both a typical month
   * and a month still running. */
  paceMark: number | null;
}

const clamp = (v: number) => (v < 0 ? 0 : v > 1 ? 1 : v);

/** 0…1 through a month still running, else null. */
export function monthProgress(view: MonthPace): number | null {
  if (!view.is_current || view.days_in_month <= 0) return null;
  return clamp(view.day / view.days_in_month);
}

export function stripBars(view: StripInput): StripBars {
  const typical = view.typical;
  const scale = Math.max(view.income, view.spent, typical ?? 0);
  const at = (v: number) => (scale > 0 ? clamp(v / scale) : 0);
  const net = view.income - view.spent;
  const pace = monthProgress(view);
  const typicalAt = typical != null ? at(typical) : null;
  return {
    income: at(view.income),
    spent: at(view.spent),
    typical: typicalAt,
    net: at(Math.abs(net)),
    negative: net < 0,
    pace,
    paceMark: pace != null && typicalAt != null ? typicalAt * pace : null,
  };
}

/** An account's month: two balances and the two flows between them. */
export interface AccountInput {
  opening: number | null;
  inflow: number;
  outflow: number;
  balance: number | null;
}

export interface AccountBars {
  /** Null where the account has no balance to draw. */
  opening: number | null;
  balance: number | null;
  inflow: number;
  outflow: number;
  /** A balance below zero still has a length; the sign is a colour. */
  openingNegative: boolean;
  balanceNegative: boolean;
}

export function accountBars(view: AccountInput): AccountBars {
  const scale = Math.max(
    Math.abs(view.opening ?? 0),
    Math.abs(view.balance ?? 0),
    view.inflow,
    view.outflow,
  );
  const at = (v: number) => (scale > 0 ? clamp(Math.abs(v) / scale) : 0);
  return {
    opening: view.opening != null ? at(view.opening) : null,
    balance: view.balance != null ? at(view.balance) : null,
    inflow: at(view.inflow),
    outflow: at(view.outflow),
    openingNegative: (view.opening ?? 0) < 0,
    balanceNegative: (view.balance ?? 0) < 0,
  };
}
