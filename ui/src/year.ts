/** The shape of a year, rather than its size.
 *
 * A ranked list of totals cannot tell a trip from a habit: 3,000 spent
 * once in November and 3,000 spread over twelve months print the same
 * number. So the card draws months — the whole year as a ribbon, then
 * every group as its own strip under the same axis — and each of them
 * carries the line that says what usual looks like, so a bar above it
 * reads as a month that cost more than it normally does.
 */

import type { YearGroup, YearMonth } from "./api";

/** Where a value sits against the tallest one, 0–1. Nothing spent all
 * year is flat, not a divide by zero. */
const share = (v: number, max: number) => (max > 0 ? v / max : 0);

/** `typical` on the same 0–1 scale as the bars beside it, or null when
 * there is no line worth drawing. A median equal to the maximum — one
 * payment, or twelve identical ones — would run along the top of every
 * bar and tell the reader nothing they cannot already see. */
const line = (typical: number | null, max: number) =>
  typical == null || max <= 0 || typical >= max ? null : typical / max;

export interface RibbonMonth {
  month: string;
  total: number;
  prior: number | null;
  /** Share of the busiest month in the ribbon, 0–1. */
  height: number;
  /** The same month a year earlier, at the same scale. Null when there
   * is no prior year, so the tick is absent rather than sitting at
   * zero and reading as "we used to spend nothing". */
  priorHeight: number | null;
  /** Cost more this month than a month of this year usually does. */
  above: boolean;
  /** Where the calendar year turns over: every January, and the first
   * month of the window, which is where its year is worth printing. */
  yearStart: boolean;
}

export interface Ribbon {
  months: RibbonMonth[];
  /** The busiest month, this year's or last year's. */
  max: number;
  /** Where a usual month sits, 0–1. */
  typicalHeight: number | null;
}

/** The whole year month by month. Last year's months count towards the
 * scale: a tick drawn at a height the ribbon cannot reach would sit
 * outside its own bar. */
export function yearRibbon(
  months: YearMonth[],
  typical: number | null,
): Ribbon {
  const max = months.reduce(
    (m, x) => Math.max(m, x.total, x.prior ?? 0),
    0,
  );
  return {
    max,
    typicalHeight: line(typical, max),
    months: months.map((m, i) => ({
      month: m.month,
      total: m.total,
      prior: m.prior,
      height: share(m.total, max),
      priorHeight: m.prior == null ? null : share(m.prior, max),
      above: typical != null && m.total > typical,
      yearStart: i === 0 || m.month.endsWith("-01"),
    })),
  };
}

export interface StripCell {
  month: string;
  value: number;
  /** Share of this group's own busiest month, 0–1. */
  height: number;
  /** Cost more this month than the group usually does. */
  above: boolean;
}

export interface Strip {
  cells: StripCell[];
  max: number;
  typicalHeight: number | null;
}

/** One group across the same axis, on its own scale: the row answers
 * when, and the amount printed beside it answers how much. Scaled
 * against the card instead, a small group would be a flat line and the
 * one month it actually happened would be invisible. */
export function yearStrip(group: YearGroup, months: YearMonth[]): Strip {
  const max = group.monthly.reduce((m, v) => Math.max(m, v), 0);
  return {
    max,
    typicalHeight: line(group.typical, max),
    cells: months.map((m, i) => {
      // Driven by the axis, not by `monthly`: a short array pads with
      // zeroes instead of sliding the rest under the wrong months.
      const value = group.monthly[i] ?? 0;
      return {
        month: m.month,
        value,
        height: share(value, max),
        above: group.typical != null && value > group.typical,
      };
    }),
  };
}

/** The groups past the cut, folded into one row that still lines up
 * with the axis. Deliberately without a typical month: rent and
 * haircuts have no shared usual cost, so the row gets no reference
 * line rather than a meaningless one. */
export function foldGroups(
  name: string,
  groups: YearGroup[],
  span: number,
): YearGroup {
  const at = (i: number) =>
    groups.reduce((s, g) => s + (g.monthly[i] ?? 0), 0);
  const monthly = Array.from({ length: span }, (_, i) => at(i));
  return {
    name,
    monthly,
    typical: null,
    total: groups.reduce((s, g) => s + g.total, 0),
    prior: groups.some((g) => g.prior != null)
      ? groups.reduce((s, g) => s + (g.prior ?? 0), 0)
      : null,
  };
}

export interface YearRow {
  group: YearGroup;
  /** Stands in for everything past the cut rather than for itself. */
  folded: boolean;
}

/** The rows the card draws, folded or opened out.
 *
 * The fold is a cut, not a summary: the groups behind it are still
 * there, so the row that hides them is the control that brings them
 * back. One group over the cut is never folded — a row reading "1 more
 * group" hides a name behind a count and saves nothing.
 */
export function yearRows(
  groups: YearGroup[],
  span: number,
  expanded: boolean,
  cut = 9,
): YearRow[] {
  const plain = (group: YearGroup): YearRow => ({ group, folded: false });
  if (expanded || groups.length <= cut + 1) return groups.map(plain);
  const rest = groups.slice(cut);
  return [
    ...groups.slice(0, cut).map(plain),
    {
      // Named for the column it sits in, which is as narrow as the
      // longest group name and no wider. The names stacked above it
      // already say what there are more of.
      group: foldGroups(`${rest.length} more`, rest, span),
      folded: true,
    },
  ];
}
