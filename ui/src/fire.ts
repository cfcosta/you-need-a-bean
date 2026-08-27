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

/** Where the lean label hangs on the bar: an offset from whichever end
 * of it the mark is nearer, so the label stands beside its tick like a
 * flag on a pole rather than centred over it.
 *
 * Centred is the obvious choice and the wrong one. A lean target is a
 * fraction of a full one, so its mark sits near the left end almost
 * always, and half a centred label would hang off the card.
 */
export function leanFlag(mark: number): { left: string } | { right: string } {
  // Percentages of a percentage: 1 - 0.8 is 0.19999999999999996, and a
  // stylesheet should not carry the proof.
  const at = (v: number) => `${Number((v * 100).toFixed(4))}%`;
  return mark > 0.5 ? { right: at(1 - mark) } : { left: at(mark) };
}
