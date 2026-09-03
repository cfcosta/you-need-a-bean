/** A rectangle in viewport coordinates — one line box, not a whole element. */
export type Box = { left: number; right: number; top: number; bottom: number };

/**
 * How many pixels two runs of text actually share, or 0 if they miss.
 *
 * Each run is the list of boxes the browser painted it into: one per
 * line. Comparing the single rectangle that bounds a run instead would
 * be wrong for anything that wraps, because that rectangle covers the
 * full width of every line it spans, including the part of the first
 * line the text does not reach and the part of the last line it never
 * gets to. Text sitting in that empty space would read as a collision
 * when the two never touch.
 *
 * `slack` is the hairline that sub-pixel layout leaves between things
 * meant to be adjacent; below it, a shared edge is not a collision.
 */
export function collide(a: Box[], b: Box[], slack = 2): number {
  let worst = 0;
  for (const p of a) {
    for (const q of b) {
      const across = Math.min(p.right, q.right) - Math.max(p.left, q.left);
      const down = Math.min(p.bottom, q.bottom) - Math.max(p.top, q.top);
      if (across > slack && down > slack && across > worst) worst = across;
    }
  }
  return Math.round(worst);
}
