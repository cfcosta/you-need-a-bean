// Typed client for the JSON endpoints the Rust binary serves.

export type Status = "good" | "warn" | "over";

export interface Summary {
  title: string | null;
  root: string | null;
  files: number;
  directives: number;
  parse_ms: number;
  operating_currencies: string[];
  months: string[];
  today: string;
  default_month: string;
  /** Counts the times the server reparsed the files; 0 is the boot load. */
  revision: number;
  /** Why the last reparse failed, if it did. The data is the last good one. */
  reload_error: string | null;
}

export interface CategoryRow {
  account: string;
  label: string;
  spent: number;
  avg: number | null;
  ratio: number | null;
  status: Status | null;
  split: Record<string, number>;
}

export interface Group {
  name: string;
  spent: number;
  avg: number | null;
  categories: CategoryRow[];
}

export interface AccountRow {
  account: string;
  label: string;
  balances: Record<string, number>;
  converted: number | null;
}

export interface MonthView {
  month: string;
  is_current: boolean;
  day: number;
  days_in_month: number;
  income: number;
  spent: number;
  typical: number | null;
  groups: Group[];
  accounts: { budget: AccountRow[]; tracking: AccountRow[] };
}

export interface Posting {
  account: string;
  amount: number | null;
  currency: string | null;
}

export interface Txn {
  date: string;
  flag: string;
  payee: string | null;
  narration: string | null;
  tags: string[];
  links: string[];
  meta: Record<string, string>;
  amount: number | null;
  currency: string | null;
  converted: number | null;
  postings: Posting[];
}

export interface CategoryView {
  account: string;
  label: string;
  spent: number;
  avg: number | null;
  ratio: number | null;
  status: Status | null;
  window: [string, string] | null;
  history: { month: string; spent: number }[];
  split: Record<string, number>;
  txns: Txn[];
}

export interface NetWorthPoint {
  month: string;
  assets: number;
  /** The budget-kind slice of `assets`: cash and its equivalents. */
  cash: number;
  /** The rest — accounts holding commodities, and hidden ones. */
  holdings: number;
  liabilities: number;
  net: number;
}

export interface CashflowPoint {
  month: string;
  income: number;
  expenses: number;
  net: number;
}

export interface FireScenario {
  rate: number;
  months: number | null;
}

/** What saving `extra` more each month does to the middle scenario. */
export interface SavingsStep {
  extra: number;
  months: number | null;
}

export interface Runway {
  liquid: number;
  /** liquid / monthly spend; null without a spend history. */
  months: number | null;
  /** The same against fixed costs alone. */
  lean_months: number | null;
}

export interface GrowthPoint {
  month: string;
  delta: number;
  /** Income − expenses: the part you saved. */
  saved: number;
  /** Capital in through `Equity:*` — opening balances, mostly. */
  equity: number;
  /** The rest: prices and rates moving under what you hold. */
  market: number;
}

export interface Growth {
  window: [string, string] | null;
  saved: number;
  equity: number;
  market: number;
  delta: number;
  /** `market` over the average net worth held; a period return. Null
   * while `unpriced` is non-empty — the split it comes from has holes. */
  implied_return: number | null;
  /** Commodities with no price, whose gaps land in `market`. */
  unpriced: string[];
  points: GrowthPoint[];
}

/** A charge whose amount stepped and stayed there. */
export interface PriceChange {
  from: number;
  to: number;
  /** What the step costs over a year at this cadence. */
  annual: number;
  since: string;
}

export interface Recurring {
  account: string;
  label: string;
  payee: string;
  /** "monthly", "yearly", … */
  cadence: string;
  amount: number;
  /** `amount` spread over a month. */
  monthly: number;
  count: number;
  first: string;
  last: string;
  /** Still charging, so still part of what next month costs. */
  active: boolean;
  change: PriceChange | null;
}

export interface RecurringView {
  window: [string, string] | null;
  /** What the still-charging series cost each month. A floor, not a
   * bill — read it next to `coverage`. */
  monthly_fixed: number;
  annual_fixed: number;
  /** `monthly_fixed` over the monthly spend. */
  coverage: number | null;
  items: Recurring[];
}

export interface Fire {
  window: [string, string] | null;
  monthly_spend: number;
  annual_spend: number;
  fire_number: number;
  net_worth: number;
  progress: number | null;
  monthly_savings: number;
  swr_monthly: number;
  scenarios: FireScenario[];
  /** The same scenarios with nothing further saved — coast FIRE. */
  coast: FireScenario[];
  steps: SavingsStep[];
  /** 25× a year of fixed costs, and progress against it. */
  lean_number: number | null;
  lean_progress: number | null;
}

export interface YearGroup {
  name: string;
  total: number;
  /** The same group over the twelve months before, or null when the
   * ledger doesn't reach back far enough to hold a whole prior year. */
  prior: number | null;
}

/** One `Income:*` group over the trailing year. */
export interface IncomeSource {
  /** The second segment, or the account itself when it has none. */
  name: string;
  total: number;
  /** `total` over all income in the window. */
  share: number;
  /** The part of `total` that arrives without work. Groups can be
   * mixed — staking rewards and a referral fee share a group. */
  passive: number;
}

export interface Income {
  window: [string, string] | null;
  total: number;
  sources: IncomeSource[];
  /** `1 / Σ share²` — how many equally-sized sources this is worth. */
  effective_sources: number | null;
  passive: number;
  /** `passive / total`. */
  passive_share: number | null;
  /** `passive` over the same window's spend: the share of the bill
   * that already pays itself. */
  passive_cover: number | null;
  /** Accounts the ledger marked `income: "passive"` outright. */
  declared: number;
  /** Accounts taken as passive from their name alone. */
  inferred: number;
}

/** What one tag or link cost, across however long it ran. */
export interface Project {
  name: string;
  /** "#" for a tag, "^" for a link. */
  sigil: string;
  /** Spend across the topic, refunds already netted out. */
  spent: number;
  /** Money that came back: a reimbursed trip didn't cost its gross. */
  income: number;
  /** `spent - income` — what it actually took. */
  net: number;
  count: number;
  /** Distinct expense accounts touched. */
  categories: number;
  first: string;
  last: string;
  /** Calendar months the span covers, inclusive. */
  months: number;
}

export interface Projects {
  items: Project[];
  /** Names on a single transaction: an importer's id, not a topic. */
  singletons: number;
  /** Names that moved no money — marks on transfers. */
  markers: number;
}

/** A category whose quarter moved against the quarter before it. */
export interface Mover {
  account: string;
  label: string;
  group: string | null;
  recent: number;
  prior: number;
  /** `recent - prior`; positive is more spending. */
  delta: number;
  /** `delta / prior`; null when nothing was spent before. */
  ratio: number | null;
}

export interface Movers {
  recent: [string, string] | null;
  prior: [string, string] | null;
  recent_total: number;
  prior_total: number;
  items: Mover[];
}

export interface ReportsView {
  month: string;
  net_worth: NetWorthPoint[];
  cashflow: CashflowPoint[];
  fire: Fire;
  runway: Runway;
  growth: Growth;
  recurring: RecurringView;
  /** Commodities nothing prices in the display currency. Every amount
   * in one of them is missing from every figure on the page. */
  unpriced: string[];
  year: {
    window: [string, string] | null;
    prior_window: [string, string] | null;
    groups: YearGroup[];
  };
  movers: Movers;
  projects: Projects;
  income: Income;
}

async function get<T>(url: string): Promise<T> {
  const res = await fetch(url);
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(
      (body as { error?: string }).error ?? `request failed: ${res.status}`,
    );
  }
  return res.json() as Promise<T>;
}

export const getSummary = () => get<Summary>("/api/summary");

export const getMonth = (month: string, basis: number, cur: string) =>
  get<MonthView>(`/api/month/${month}?basis=${basis}&cur=${cur}`);

export const getCategory = (
  account: string,
  month: string,
  basis: number,
  cur: string,
) =>
  get<CategoryView>(
    `/api/category/${encodeURIComponent(account)}/${month}?basis=${basis}&cur=${cur}`,
  );

export const getReports = (basis: number, cur: string) =>
  get<ReportsView>(`/api/reports?basis=${basis}&cur=${cur}`);
