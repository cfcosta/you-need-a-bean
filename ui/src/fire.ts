/** The two positions on the independence bar.
 *
 * The card used to spell the lean number out in a sentence under the
 * bar, which left the reader holding two figures and working out how
 * far apart they were. They are both distances along the same line, so
 * the bar draws them both and the sentence goes away.
 */

/** How much of the bar is already paid for, 0–1. Progress past the
 * target keeps filling nothing, and progress below zero — a net worth
 * in the red — leaves the bar empty rather than drawing it backwards. */
export function fireFill(progress: number | null): number {
  return Math.max(0, Math.min(1, progress ?? 0));
}

/** Where the lean target stands on the way to the full one, 0–1, or
 * null when there is no second goalpost worth marking: a lean number
 * at or past the target would put the marker on the target's own edge
 * and read as the thing it is standing beside. */
export function leanMark(lean: number | null, fire: number): number | null {
  if (lean == null || lean <= 0 || fire <= 0 || lean >= fire) return null;
  return lean / fire;
}
