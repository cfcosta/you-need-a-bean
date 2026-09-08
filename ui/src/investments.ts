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

export type PerformancePeriod = "1M"|"3M"|"6M"|"YTD"|"1Y"|"All";
/** Opening is end-of-day, so YTD includes every transaction on January 1. */
export function performanceStart(today:string, period:PerformancePeriod):string|null {
  if(period==="All")return null;
  const [year,month,day]=today.split("-").map(Number) as [number,number,number];
  if(period==="YTD")return `${year-1}-12-31`;
  const back=period==="1Y"?12:Number(period.slice(0,-1));
  const d=new Date(Date.UTC(year,month-1-back,1));
  const last=new Date(Date.UTC(d.getUTCFullYear(),d.getUTCMonth()+1,0)).getUTCDate();
  d.setUTCDate(Math.min(day,last));
  return d.toISOString().slice(0,10);
}

/** Discontinuous paths keep unknown valuations visible as gaps, never zeroes. */
export function performanceChart(rows:import("./api").PerformancePoint[], opening:number|null) {
  const start=Date.parse(rows[0]?.date??"1970-01-01");
  const end=Date.parse(rows.at(-1)?.date??"1970-01-01");
  const capital=(p:import("./api").PerformancePoint)=>opening==null||p.net_flows==null?null:opening+p.net_flows;
  const values=rows.flatMap(p=>[p.value,capital(p)]).filter((n):n is number=>n!=null);
  const min=Math.min(0,...values),max=Math.max(1,...values),span=max-min;
  const y=(n:number)=>220-(n-min)/span*200;
  const points=rows.map(p=>({...p,x:(Date.parse(p.date)-start)/Math.max(1,end-start)*1000,y:p.value==null?null:y(p.value),capital:capital(p)}));
  const path=(field:"value"|"capital")=>{let known=false;return points.map(p=>{const n=p[field];if(n==null){known=false;return "";}const command=known?"L":"M";known=true;return `${command}${p.x.toFixed(2)},${y(n).toFixed(2)}`;}).join(" ");};
  return {points,valuePath:path("value"),capitalPath:path("capital"),min,max,y};
}
