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
