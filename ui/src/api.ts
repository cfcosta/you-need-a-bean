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
}

export interface ReportsView {
  month: string;
  net_worth: NetWorthPoint[];
  cashflow: CashflowPoint[];
  fire: Fire;
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
