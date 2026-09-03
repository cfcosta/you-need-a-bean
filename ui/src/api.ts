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

/** A file a `document` directive attached to an account on a day. */
export interface Doc {
  id: number;
  /** File name, which is what a person recognises it by. */
  name: string;
  /** Where it sits on disk, for whoever wants to go and find it. */
  path: string;
  /** The account the directive hung it off — not always this one. */
  account: string;
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
  documents: Doc[];
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

/** One month of an account's register, for the little chart. */
export interface AccountPoint {
  month: string;
  inflow: number;
  outflow: number;
  /** What it closed on; null when something it holds has no price. */
  balance: number | null;
}

/** A register line: the transaction, and where it left the account. */
export interface RegisterTxn extends Txn {
  /** What it did to this account. Null when a leg has no price. */
  delta: number | null;
  /** The balance it left behind, null from the first hole onwards. */
  balance: number | null;
}

export interface AccountView {
  account: string;
  label: string;
  kind: "budget" | "tracking" | "hidden";
  /** The balance the month opened on. */
  opening: number | null;
  inflow: number;
  outflow: number;
  balance: number | null;
  balances: Record<string, number>;
  history: AccountPoint[];
  /** Commodities this month moved that nothing prices, and so are
   * missing from every figure above. */
  unpriced: string[];
  txns: RegisterTxn[];
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

/** What the charges on a steady cadence add up to. Not a report of
 * its own — it sizes the lean FIRE target and the lean runway, which
 * is why only the totals travel. */
export interface RecurringView {
  window: [string, string] | null;
  /** What the still-charging series cost each month. A floor, not a
   * bill — read it next to `coverage`. */
  monthly_fixed: number;
  annual_fixed: number;
  /** `monthly_fixed` over the monthly spend. */
  coverage: number | null;
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
  /** Spend in each of `year.months`, in that order and always that
   * long, so a cell always sits under the month it belongs to. */
  monthly: number[];
  /** What one month of this group usually costs — the median of the
   * months it was actually paid. Null when it never was. */
  typical: number | null;
}

/** One month of the trailing year, across every expense group. */
export interface YearMonth {
  month: string;
  total: number;
  /** The same calendar month a year earlier, or null without a whole
   * prior year to take it from. */
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
  /** Ranked the same way, but past the cut — the card opens onto
   * these rather than pretending the list above was all of them. */
  hidden: Mover[];
}

/** One calendar month, across every year the ledger covers. */
export interface SeasonPoint {
  /** 1 through 12. */
  month: number;
  /** Median spend in this calendar month over the sample years. */
  median: number;
  /** `median` over the twelve medians: the shape, level divided out. */
  share: number;
  /** How many years fed the median. One is not a median. */
  samples: number;
  /** What this year actually spent, once the month is over. */
  actual: number | null;
}

export interface Season {
  /** The calendar years behind the medians, inclusive. Null when the
   * ledger doesn't cover all twelve months even once. */
  years: [number, number] | null;
  /** Twelve points in calendar order; empty when `years` is null. */
  months: SeasonPoint[];
  /** What a median year costs: the twelve medians added up. */
  typical: number;
  year: number;
  /** Months of `year` that are over, and what they cost. */
  elapsed: number;
  ytd: number;
  /** `ytd` over what those same months usually cost. Above one is a
   * year running hot, measured against the same months. */
  pace: number | null;
  /** `ytd` plus the rest of the year at this year's pace. */
  projected: number | null;
}

/** One commodity still held, at what it is worth and what it cost.
 *
 * `basis`, `gain` and `ret` are null together. A position reports a
 * cost only when every unit currently held arrived carrying one, so a
 * holding that came in as staking interest or a swap says nothing
 * rather than claiming it was free. */
export interface Position {
  /** The ticker, as the ledger writes it. */
  currency: string;
  /** What a `commodity` directive named it, or the ticker again. */
  label: string;
  /** Its declared `asset-class:`, or null where nothing declared one. */
  class: string | null;
  units: number;
  price: number;
  value: number;
  /** `value` over the whole portfolio, dust included. */
  share: number;
  basis: number | null;
  gain: number | null;
  ret: number | null;
  /** Accounts still holding it, and postings that ever touched it. */
  accounts: number;
  postings: number;
  first: string;
  last: string;
}

/** What one declared asset class adds up to. */
export interface AssetClass {
  /** null for the commodities nothing classified. */
  name: string | null;
  value: number;
  share: number;
  positions: number;
}

export interface Investments {
  /** Ranked by value, biggest first, with dust folded out. */
  items: Position[];
  /** Ranked the same way, unclassified last. */
  classes: AssetClass[];
  /** Everything held, dust and unclassified included. */
  total: number;
  /** Positions too small to rank, and what they came to together. */
  dust: Position[];
  dust_value: number;
  /** What the positions that recorded a cost cost, what those same
   * positions are worth now, and the gain between them. */
  basis: number;
  based_value: number;
  gain: number;
  ret: number | null;
  /** `based_value / total` — how much of the portfolio that gain
   * actually speaks for. */
  coverage: number | null;
  /** Held, valued, and with nothing to compare against. */
  unbased: number;
  unbased_count: number;
  /** How many equally-sized holdings this spread is worth, which is
   * the number the count only looks like. */
  effective: number | null;
  /** Commodities held that nothing prices in the display currency.
   * They are missing from every figure above. */
  unpriced: string[];
}

/** One merchant, over the trailing year. */
export interface Payee {
  /** The name the ledger writes, trimmed and no further. */
  name: string;
  spent: number;
  /** Charges behind `spent`, and what one of them averages. */
  count: number;
  average: number;
  /** `spent` over the window's named and unnamed spend together. */
  share: number;
  /** Expense categories and distinct months this name reaches. One
   * category over twelve months is a subscription; many over two is a
   * shop you happen to buy everything at. */
  categories: number;
  months: number;
  first: string;
  last: string;
}

export interface Payees {
  window: [string, string] | null;
  /** Ranked by spend, biggest first, capped at fifteen. */
  items: Payee[];
  /** Every expense in the window, named or not — the denominator of
   * `share`. */
  total: number;
  /** The names that qualified but didn't fit, and what they took. */
  others: Payee[];
  others_spent: number;
  /** Spend on transactions that name no payee, which is spend this
   * card cannot rank rather than spend that didn't happen. */
  anonymous: number;
  anonymous_count: number;
}

/** A commodity still held, valued at a price from a while ago. */
export interface StalePrice {
  commodity: string;
  /** The price date the current valuation actually used. */
  last: string;
  days: number;
  /** What that price is holding up, in the display currency. */
  value: number;
}

/** One transaction the ledger marks `!`. */
export interface FlaggedTxn {
  date: string;
  payee: string | null;
  narration: string | null;
  /** What it moved: the sum of its positive postings. */
  amount: number;
}

export interface Trust {
  /** The trailing year the money figures cover. */
  window: [string, string] | null;
  /** Held commodities priced more than six weeks ago, most money
   * first. */
  stale: StalePrice[];
  flagged: {
    /** Every `!` in the ledger, however old. */
    total: number;
    /** The ones inside the window, which the page is built from. */
    window: number;
    amount: number;
    /** Newest first. */
    recent: FlaggedTxn[];
  };
  uncategorized: {
    total: number;
    /** `total` over all window spend. */
    share: number | null;
    accounts: string[];
  };
  /** What the loader said while reading the files. */
  warnings: string[];
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
    /** The window's months in order, oldest first: the axis every
     * group's `monthly` is indexed by. */
    months: YearMonth[];
    /** What a month of this year usually costs. */
    typical: number | null;
    groups: YearGroup[];
  };
  movers: Movers;
  projects: Projects;
  income: Income;
  season: Season;
  investments: Investments;
  payees: Payees;
  trust: Trust;
}

export type DebtKind = "installment" | "revolving";

/** One payment on a debt, split the way the ledger split it. */
export interface DebtPayment {
  date: string;
  total: number;
  /** What came off the balance. */
  principal: number;
  /** What went to an interest expense in the same transaction. */
  interest: number;
}

/** What a debt stood at when a month ended. Null when the balance
 * could not be converted that month. */
export interface DebtPoint {
  month: string;
  owed: number | null;
}

/** One movement on a debt inside the history window: the balance it
 * left behind and how much it moved it. The first point is the
 * balance at the window's start, with no movement. */
export interface TrailPoint {
  date: string;
  owed: number | null;
  /** Positive when the debt grew. */
  delta: number | null;
}

/** Where the current payment leads, at the current rate. */
export interface Payoff {
  months: number;
  month: string;
  /** Interest still to come. */
  interest: number;
}

/** A card's month: what went on it, what came off it, and what was
 * left over from before. */
export interface Cycle {
  charges: number;
  payments: number;
  /** What the last payment did not clear. Null when the card could not
   * be read that closely. */
  carried: number | null;
  in_full: boolean | null;
}

/** A slice of a debt held in a currency other than the report's. */
export interface Foreign {
  code: string;
  /** Positive when money is owed. */
  amount: number;
  /** In the report's currency, or null when the ledger has no price. */
  converted: number | null;
}

/** What a loan is secured on, and what the ledger says it is worth. */
export interface Collateral {
  account: string;
  label: string;
  value: number | null;
}

export interface Debt {
  account: string;
  label: string;
  kind: DebtKind;
  /** Positive when money is owed. */
  owed: number;
  balances: Record<string, number>;
  foreign: Foreign[];
  /** The credit limit the account declares, in the report's currency. */
  limit: number | null;
  /** owed / limit, floored at zero. */
  utilisation: number | null;
  collateral: Collateral | null;
  /** The most that was ever owed. */
  peak: number;
  /** How much of the peak is paid off; loans only. */
  progress: number | null;
  /** Yearly, read off the interest the ledger charged. */
  rate: number | null;
  /** The usual payment. */
  payment: number | null;
  due_day: number | null;
  next_due: string | null;
  principal_paid: number;
  interest_paid: number;
  /** Newest first. */
  payments: DebtPayment[];
  /** Month ends, oldest first. */
  history: DebtPoint[];
  trail: TrailPoint[];
  payoff: Payoff | null;
  cycle: Cycle | null;
  /** A carried card against its own new charges. */
  treadmill: Treadmill | null;
  /** What a carried balance is made of. */
  makeup: Makeup | null;
}

/** One month of a carried card's treadmill: what went on it, what
 * came off it, and the interest in between. */
export interface TreadmillMonth {
  month: string;
  /** New charges, interest left out. */
  charges: number;
  payments: number;
  interest: number;
}

/** A carried card measured against what keeps going on it. The
 * card's `payoff` pays the balance down as if nothing more were
 * charged; this reads the net, and says where that really leads. */
export interface Treadmill {
  /** Oldest first, over the basis. */
  months: TreadmillMonth[];
  /** Complete months the pace is read over. */
  pace: number;
  charged: number;
  paid: number;
  /** Paid less charged, a month; negative when the card is growing. */
  net: number;
  /** Where `net` a month leads; null when it never gets there. */
  payoff: Payoff | null;
}

/** A slice of a carried balance: the oldest unpaid charges to one
 * expense account, and the interest they have run up. */
export interface MakeupRow {
  /** Empty when the charge said nothing about what it was for. */
  account: string;
  label: string;
  owed: number;
  charged: number;
  /** Accrued on these charges so far, paid or not. */
  interest: number;
  since: string;
  count: number;
}

export interface Makeup {
  /** Biggest first. */
  rows: MakeupRow[];
  total: number;
}

/** What is safe to send over the usual payments: this month, from the
 * cash on hand, and each month, from what is usually left over. */
export interface Extra {
  cash: number;
  /** Due in the next 31 days. */
  due: number;
  /** A typical month of cash-paid spending. */
  spend: number;
  /** A month of the fixed costs, kept back. */
  buffer: number;
  /** cash − due − spend − buffer, floored at zero. */
  now: number;
  /** The usual month's surplus after every payment, floored at zero. */
  monthly: number;
}

/** Whether the cash on hand covers what is on the cards. */
export interface Cover {
  cash: number;
  owed: number;
  covered: boolean;
  after: number;
}

export interface Upcoming {
  account: string;
  label: string;
  date: string;
  amount: number;
}

export type NoticeKind = "missed" | "growing" | "overpaid";

/** Something the figures do not say on their own. */
export interface DebtNotice {
  kind: NoticeKind;
  account: string;
  label: string;
  amount: number | null;
  day: number | null;
}

/** A debt that reached zero: the receipt for it. */
export interface Beaten {
  account: string;
  label: string;
  peak: number;
  principal_paid: number;
  interest_paid: number;
  /** The first movement on it and the last payment. */
  first: string;
  last: string;
}

export interface LiabilitiesView {
  month: string;
  owed: number;
  installment: number;
  revolving: number;
  interest: {
    month: number;
    /** Over `window`. */
    year: number;
    earned_month: number;
    earned_year: number;
    window: [string, string] | null;
  };
  /** A year of interest at today's balances and rates. */
  cost_year: number;
  blended_rate: number | null;
  /** When the last loan is paid off, at today's payments. */
  debt_free: string | null;
  cover: Cover;
  extra: Extra;
  upcoming: Upcoming[];
  notices: DebtNotice[];
  /** The yearly return a portfolio is assumed to make, for a loan's
   * rate to stand against. */
  assumed_return: number;
  /** Most owed first. */
  debts: Debt[];
  /** Most recently paid off first. */
  beaten: Beaten[];
  unpriced: string[];
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

export const getAccount = (
  account: string,
  month: string,
  basis: number,
  cur: string,
) =>
  get<AccountView>(
    `/api/account/${encodeURIComponent(account)}/${month}?basis=${basis}&cur=${cur}`,
  );

export const getReports = (basis: number, cur: string) =>
  get<ReportsView>(`/api/reports?basis=${basis}&cur=${cur}`);

export const getLiabilities = (basis: number, cur: string) =>
  get<LiabilitiesView>(`/api/liabilities?basis=${basis}&cur=${cur}`);
