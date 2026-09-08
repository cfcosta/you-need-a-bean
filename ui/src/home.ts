export interface CashPoint { date: string; balance: number }
const DAY = 86_400_000;
export function scenario(points: CashPoint[], today: string, horizon: number, monthly: number): CashPoint[] {
  if (!points.length) return [];
  const start = Date.parse(today + "T00:00:00Z");
  const end = new Date(start + horizon * DAY).toISOString().slice(0,10);
  const extended = [...points];
  if (extended.at(-1)!.date < end) extended.push({date:end,balance:extended.at(-1)!.balance});
  return extended.map(p => ({date:p.date,balance: Math.round((p.balance - monthly * (Date.parse(p.date + "T00:00:00Z")-start)/DAY/30)*100)/100}));
}
export const accountUrl = (account:string, day:string) => `/account/${encodeURIComponent(account)}/${day.slice(0,7)}`;
export const dayLabel = (day:string) => new Date(day+"T12:00:00Z").toLocaleDateString("en-US",{month:"short",day:"numeric",timeZone:"UTC"});

/** Financial days follow the reader's calendar, not the UTC date. */
export const localDay = (d = new Date()) => `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,"0")}-${String(d.getDate()).padStart(2,"0")}`;
