// Generates a large synthetic multi-file beancount ledger for performance
// testing. Deterministic (mulberry32), 120 months x N txns; the documented
// corpus is `bun tools/perf-gen.ts /tmp/big 2100` (~252k directives,
// ~26 MB), served with
// `you-need-a-bean /tmp/big/main.beancount`.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const OUT = Bun.argv[2] ?? "./big";
const MONTHS = 120; // 2016-09 .. 2026-08
const TXNS_PER_MONTH = Number(Bun.argv[3] ?? 700);

function mulberry32(seed: number) {
  return () => {
    seed |= 0; seed = (seed + 0x6d2b79f5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
const rnd = mulberry32(20260821);
const pick = <T,>(a: T[]) => a[Math.floor(rnd() * a.length)]!;
const amt = (lo: number, hi: number) => (lo + rnd() * (hi - lo)).toFixed(2);

const expenses = [
  ["Expenses:Food:Groceries", "Groceries", ["SuperMart", "Whole Foods", "Corner Deli", "Feira Livre"]],
  ["Expenses:Food:Restaurants", "Eating Out", ["Osteria", "Sushi Aki", "Taqueria Norte", "Padaria Estrela"]],
  ["Expenses:Food:Coffee", "Coffee", ["Blue Bottle", "Cafe Rio", "Starbucks"]],
  ["Expenses:Home:Rent", "Rent", ["Landlord"]],
  ["Expenses:Home:Utilities", "Utilities", ["Con Edison", "Water Co"]],
  ["Expenses:Home:Internet", "Internet", ["Fiber ISP"]],
  ["Expenses:Transport:Fuel", "Fuel", ["Shell", "BP"]],
  ["Expenses:Transport:Rideshare", "Rideshare", ["99", "Uber"]],
  ["Expenses:Transport:Transit", "Transit", ["MTA"]],
  ["Expenses:Health:Pharmacy", "Pharmacy", ["Duane Reade"]],
  ["Expenses:Health:Insurance", "Health Insurance", ["Blue Cross"]],
  ["Expenses:Fun:Streaming", "Streaming", ["Netflix", "Spotify"]],
  ["Expenses:Fun:Games", "Games", ["GameStore", "Steam"]],
  ["Expenses:Fun:Books", "Books", ["Bookshop"]],
  ["Expenses:Clothes:Everyday", "Clothes", ["Uniqlo", "Zara"]],
  ["Expenses:Travel:Flights", "Flights", ["LATAM", "United"]],
  ["Expenses:Travel:Hotels", "Hotels", ["Marriott"]],
  ["Expenses:Taxes:Federal", "Federal Tax", ["IRS"]],
] as const;
const assets = ["Assets:US:Chase:Checking", "Assets:BR:Nubank:Conta", "Liabilities:US:Amex:Blue", "Liabilities:US:Chase:Slate"];

mkdirSync(join(OUT, "months"), { recursive: true });

let accounts = "";
for (const a of assets) accounts += `2016-09-01 open ${a}  ${a.includes(":BR:") ? "BRL" : "USD"}\n  name: "${a.split(":").slice(-2).join(" ")}"\n`;
accounts += `2016-09-01 open Income:US:Acme:Salary  USD\n  name: "Acme Salary"\n`;
accounts += `2016-09-01 open Income:BR:Freela  BRL\n  name: "Freelance BR"\n`;
accounts += `2016-09-01 open Assets:US:Vanguard:VTI  VTI\n  name: "Vanguard Index"\n  ynab: "tracking"\n`;
for (const [acct, name] of expenses) accounts += `2016-09-01 open ${acct}  USD, BRL\n  name: "${name}"\n`;
writeFileSync(join(OUT, "accounts.beancount"), accounts);

let prices = "";
let directiveCount = 0;
const monthFiles: string[] = [];
for (let i = 0; i < MONTHS; i++) {
  const y = 2016 + Math.floor((8 + i) / 12);
  const m = ((8 + i) % 12) + 1;
  const mm = String(m).padStart(2, "0");
  const daysIn = new Date(Date.UTC(y, m, 0)).getUTCDate();
  const rate = (4.8 + rnd()).toFixed(2);
  prices += `${y}-${mm}-${String(daysIn).padStart(2, "0")} price USD ${rate} BRL\n`;
  prices += `${y}-${mm}-${String(daysIn).padStart(2, "0")} price VTI ${(180 + i + rnd() * 10).toFixed(2)} USD\n`;
  directiveCount += 2;

  let f = `;; ${y}-${mm} (generated)\n\n`;
  f += `${y}-${mm}-01 * "Landlord" "rent"\n  Expenses:Home:Rent  ${amt(1700, 1900)} USD\n  Assets:US:Chase:Checking\n\n`;
  f += `${y}-${mm}-${String(Math.min(daysIn, 28)).padStart(2, "0")} * "Acme Corp" "salary"\n  Income:US:Acme:Salary  -${amt(6000, 7000)} USD\n  Assets:US:Chase:Checking\n\n`;
  directiveCount += 2;
  for (let t = 0; t < TXNS_PER_MONTH; t++) {
    const [acct, , payees] = pick(expenses as unknown as any[]);
    const day = String(1 + Math.floor(rnd() * daysIn)).padStart(2, "0");
    const brl = rnd() < 0.18;
    const src = brl ? "Assets:BR:Nubank:Conta" : pick(assets.filter((a) => !a.includes(":BR:")));
    const tag = rnd() < 0.08 ? " #recurring" : "";
    const meta = rnd() < 0.05 ? `\n  note: "auto-generated line ${t}"` : "";
    f += `${y}-${mm}-${day} * "${pick(payees)}" "purchase ${t}"${tag}${meta}\n  ${acct}  ${amt(4, brl ? 400 : 220)} ${brl ? "BRL" : "USD"}\n  ${src}\n\n`;
    directiveCount++;
  }
  const name = `months/${y}-${mm}.beancount`;
  writeFileSync(join(OUT, name), f);
  monthFiles.push(name);
}
writeFileSync(join(OUT, "prices.beancount"), prices);

writeFileSync(
  join(OUT, "main.beancount"),
  `option "title" "Perf Torture Ledger"\noption "operating_currency" "USD"\noption "operating_currency" "BRL"\n\ninclude "accounts.beancount"\ninclude "prices.beancount"\ninclude "months/*.beancount"\n`,
);
console.log(`wrote ${OUT}: ${MONTHS} month files, ~${directiveCount + 25} directives`);
