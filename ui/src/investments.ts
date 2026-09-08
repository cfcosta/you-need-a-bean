import type { Position } from "./api";
import { fmt } from "./format";

export type HoldingSort = "value" | "gain" | "name" | "quote";

/** Class values are JSON encoded so a declared class never collides with All. */
export function selectHoldings(rows: Position[], query: string, assetClass: string, sort: HoldingSort): Position[] {
  const terms = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const result = rows.filter(p => {
    if (assetClass !== "all" && JSON.stringify(p.class) !== assetClass) return false;
    const text = [p.currency, p.label, p.class ?? "unclassified", ...p.locations.flatMap(a => [a.account, a.label])].join(" ").toLowerCase();
    return terms.every(term => text.includes(term));
  });
  return result.sort((a,b) => {
    let order = 0;
    if (sort === "value") order = b.value - a.value;
    if (sort === "gain") order = a.gain == null ? (b.gain == null ? 0 : 1) : b.gain == null ? -1 : b.gain-a.gain;
    if (sort === "name") order = a.label.localeCompare(b.label);
    if (sort === "quote") order = (a.price_date ?? "").localeCompare(b.price_date ?? "");
    return order || a.currency.localeCompare(b.currency);
  });
}

export function holdingPrice(price: number, cur: string): string {
  const n = Math.abs(price);
  if (n > 0 && n < 1e-8) return `${price.toPrecision(4)} ${cur}`;
  if (n > 0 && n < 1) return `${price.toLocaleString("en-US", {maximumSignificantDigits:4})} ${cur}`;
  return fmt(price, cur);
}

export function quoteAge(priceDate: string | null, today: string): number | null {
  if (priceDate == null) return null;
  return Math.max(0, Math.round((Date.parse(today+"T00:00:00Z") - Date.parse(priceDate+"T00:00:00Z")) / 86_400_000));
}
