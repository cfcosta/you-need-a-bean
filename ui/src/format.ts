// Number and month formatting shared across the app. Amounts keep the
// mockup's shape: "R$ 1,234.50" with a true minus sign, or the raw
// commodity code when no symbol is known.

const SYM: Record<string, string> = {
  USD: "$",
  BRL: "R$",
  EUR: "€",
  GBP: "£",
  JPY: "¥",
  CAD: "CA$",
  AUD: "A$",
  MXN: "MX$",
  INR: "₹",
  KRW: "₩",
};

const nf2 = new Intl.NumberFormat("en-US", {
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});
const nf0 = new Intl.NumberFormat("en-US", { maximumFractionDigits: 0 });

export function fmt(n: number, cur: string, dec: 0 | 2 = 2): string {
  const s = (dec === 0 ? nf0 : nf2).format(Math.abs(n));
  return (n < 0 ? "−" : "") + (SYM[cur] ?? cur) + " " + s;
}

export function fmtCode(n: number, cur: string): string {
  return (n < 0 ? "−" : "") + nf2.format(Math.abs(n)) + " " + cur;
}

/** "R$ 1.2M" — compact money for chart axes and last-point labels. */
export function fmtCompact(n: number, cur: string): string {
  const a = Math.abs(n);
  const short = (v: number, suffix: string) =>
    (v >= 100 ? v.toFixed(0) : v.toFixed(1).replace(/\.0$/, "")) + suffix;
  const s =
    a >= 1e9
      ? short(a / 1e9, "B")
      : a >= 1e6
        ? short(a / 1e6, "M")
        : a >= 1e3
          ? short(a / 1e3, "k")
          : nf0.format(a);
  return (n < 0 ? "−" : "") + (SYM[cur] ?? cur) + " " + s;
}

const MONTHS = [
  "January",
  "February",
  "March",
  "April",
  "May",
  "June",
  "July",
  "August",
  "September",
  "October",
  "November",
  "December",
];

/** "2026-08" → "August 2026" */
export function monthName(m: string): string {
  return `${MONTHS[Number(m.slice(5, 7)) - 1]} ${m.slice(0, 4)}`;
}

/** 1 through 12 → "January". A calendar month with no year attached,
 * for the shape a year has rather than a month in one. */
export function calendarMonth(n: number): string {
  return MONTHS[n - 1] ?? "";
}

/** "2026-08" → "Aug" */
export function monthShort(m: string): string {
  return (MONTHS[Number(m.slice(5, 7)) - 1] ?? "").slice(0, 3);
}

/** The averaging window as "Dec – Jan 2026", or "—" when there is none. */
export function windowLabel(window: [string, string] | null): string {
  if (!window) return "—";
  const [from, to] = window;
  if (from === to) return `${monthShort(to)} ${to.slice(0, 4)}`;
  return `${monthShort(from)} – ${monthShort(to)} ${to.slice(0, 4)}`;
}

/** spent/avg ratio as "93%", or "—" when there is no usable target. */
export function pctLabel(ratio: number | null): string {
  return ratio == null ? "—" : `${Math.round(ratio * 100)}%`;
}

/** Trailing zeros a fixed width added, and the dot they leave behind. */
const trimZeros = (s: string) =>
  s.includes(".") ? s.replace(/\.?0+$/, "") : s;

/** A share as a percentage, at whatever precision the number deserves:
 * whole percent once rounding cannot lie about it, one decimal through
 * the single digits, and two significant figures below one percent.
 *
 * The last case is the point. Flooring everything small to "<1%" makes
 * two different ratios print as the same string, so a line carrying
 * both reads as though it repeats itself — and a number that is small
 * is not the same as a number nobody measured.
 */
export function ratio(v: number | null): string {
  if (v == null) return "—";
  const p = Math.abs(v) * 100;
  if (p === 0) return "0%";
  const sign = v < 0 ? "−" : "";
  // Below a millionth of a percent `toPrecision` reaches for exponent
  // notation, which is not a number anyone reads off a card.
  if (p < 1e-6) return `${sign}<0.000001%`;
  const s = p >= 10 ? p.toFixed(0) : p >= 1 ? p.toFixed(1) : p.toPrecision(2);
  return `${sign}${trimZeros(s)}%`;
}

/** "2026-08" → "Aug 2026" */
export const monthYear = (m: string) => `${monthShort(m)} ${m.slice(0, 4)}`;

/** 27 → "2 yr 3 mo" */
export function duration(months: number): string {
  const y = Math.floor(months / 12);
  const m = months % 12;
  if (y === 0) return `${m} mo`;
  if (m === 0) return `${y} yr`;
  return `${y} yr ${m} mo`;
}
