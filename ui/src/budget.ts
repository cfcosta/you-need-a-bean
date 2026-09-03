import type { WhenSpent } from "./api";

/** The lands-on figure worth showing beside the average, or null when
 * it is not worth showing. A category that lands every month answers
 * both questions with the same number, so its cell stays empty rather
 * than printing the average twice; one that never landed has no such
 * month to price. `of` is the window it landed in that many months of,
 * which is what makes the amount readable. */
export function lands(
  when: WhenSpent | null,
  windowMonths: number,
): { amount: number; months: number; of: number } | null {
  if (when == null || windowMonths <= 0) return null;
  if (when.months >= windowMonths) return null;
  return { amount: when.amount, months: when.months, of: windowMonths };
}
