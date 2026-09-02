/**
 * The averaging window for a month: the `basis` months strictly before it,
 * clamped to the ledger's first month. Mirrors the server's clamping so the
 * "avg of …" labels match the numbers.
 */
export function monthWindow(
  months: string[],
  month: string,
  basis: number,
): [string, string] | null {
  const i = months.indexOf(month);
  if (i <= 0) return null;
  const from = months[Math.max(0, i - basis)];
  const to = months[i - 1];
  return from != null && to != null ? [from, to] : null;
}

/** "2026-08" plus n months, which may be negative. */
export function addMonths(m: string, n: number): string {
  const total = Number(m.slice(0, 4)) * 12 + (Number(m.slice(5, 7)) - 1) + n;
  const y = Math.floor(total / 12);
  return `${y}-${String(total - y * 12 + 1).padStart(2, "0")}`;
}
