/** The scale work every chart repeats. */

/** Round gridline values covering [min, max], roughly `want` of them. */
export function ticks(min: number, max: number, want: number): number[] {
  const raw = (max - min) / Math.max(1, want);
  if (!(raw > 0)) return [];
  const mag = 10 ** Math.floor(Math.log10(raw));
  const r = raw / mag;
  const step = (r >= 5 ? 10 : r >= 2 ? 5 : r >= 1 ? 2 : 1) * mag;
  const out: number[] = [];
  for (let v = Math.ceil(min / step) * step; v <= max; v += step) out.push(v);
  return out;
}

/** Pad [min, max] so marks don't touch the frame; always spans zero. */
export function bounds(values: number[]): [number, number] {
  let min = Math.min(0, ...values);
  let max = Math.max(0, ...values);
  if (max === min) max = min + 1;
  const head = (max - min) * 0.08;
  if (min < 0) min -= head;
  return [min, max + head];
}
