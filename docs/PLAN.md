# you need a bean — implementation plan

A read-only financial viewer for Beancount ledgers. Point the native app at
a ledger and it opens the overview in a desktop window:

```
you-need-a-bean examples/example.beancount
```

The interface uses the Plain Text design language: one monospace face,
a 14/22 rhythm, outlined sections, and figures written as the sums they
represent. It supports Tokyo Night day and night palettes and switches to
a phone layout below 640px.

## Architecture

```
crates/core     you-need-a-bean-core     pure library: load → model → queries
crates/desktop  you-need-a-bean-desktop  binary "you-need-a-bean": the GPUI app
```

Data flow: parse all files once at startup into an immutable `Ledger`
(indexed aggregates), share it behind an `Arc`, and answer every financial
view directly from the in-memory model. The app polls the ledger's files and
rebuilds the model in the background when they change. The SQL console lazily
loads the same reading into an in-memory DuckDB database when first opened.

- **Parser crate**: `beancount-parser` (nom-based, fast, metadata/tags/links
  support, generic over the decimal type). We use `rust_decimal::Decimal`
  internally.
- **UI**: gpui-pre, drawn from the Plain Text design canvas; headless visual
  tests compare every page with committed day, night, desktop, and phone
  references.

## Ingestion rules

- **Includes**: `include "path"` resolved relative to the including file's
  directory; absolute paths allowed; glob patterns supported (beancount v3
  parity). Files are deduplicated by canonical path; cycles are safe.
- **Options**: `operating_currency` (repeatable, ordered), `title`.
- **Directives used**: transactions (flag, payee, narration, tags, links,
  metadata, postings), `open` (account currencies + metadata), `price`,
  `option`, `include`. Others parse but are ignored for now.
- **Pretty names**: `name: "…"` metadata on an `open` directive. Fallback
  labels are derived from the account name (see below).

## Computation rules (these become the unit tests)

- **Month key** = `YYYY-MM` of the transaction date. Month range =
  `[first txn month, max(last txn month, current month)]`; the default
  selected month is today's month clamped into that range.
- **Categories** = accounts under `Expenses:` that receive postings.
  Group = second segment (`Expenses:Food:Groceries` → `Food`;
  `Expenses:Vacation` → `Vacation`). Fallback label = segments after the
  group joined with " · " (or the group itself at depth 2).
- **Spent** (per category, month, currency) = sum of posting amounts,
  expenses-positive; refunds subtract. All flags count.
- **Average ("typical")** with basis N at month M = mean of monthly spend
  over the N calendar months strictly before M, clamped to the ledger's
  first activity month. Missing months inside the window count as 0.
  Empty window → no target (UI shows "—", no percentage), and so is an
  average of zero: there is nothing to measure the month against.
- **When it lands** = the mean of the window months whose spend was
  above 0, with how many of them there were. It answers what a category
  costs when the money actually goes out, where the average spreads the
  same money over the quiet months too; a refunded month is not a month
  it landed in, though the average still lets the refund pull it down.
  The table shows it only where the two differ — a category that lands
  every month would just print its average twice — so the column is
  blank down the regular rows and filled down the lumpy ones.
- **Status**: ratio r = spent/avg; `over` if r > 1, `warn` if r > 0.85,
  else `good`.
- **Conversion** to the display currency at date D (end of selected month):
  direct price ≤ D, else inverse price ≤ D, else one-hop pivot through an
  operating currency. Unconvertible amounts are excluded from converted
  totals but always shown in the native split (e.g. `VACHR`).
- **Income tile** = −(sum of postings to `Income:*`) for the month, converted.
- **Account balances** = cumulative postings from ledger start through the
  end of the selected month, per commodity. An account is **tracking** if
  it holds any non-operating-currency commodity (e.g. stock lots), else
  **budget**; `ynab: "budget" | "tracking" | "hidden"` open-metadata
  overrides. Budget accounts = `Assets:*` + `Liabilities:*`.
- **Asset/liability fallback labels**: last two segments joined with a
  space when depth > 2 (`Assets:US:BofA:Checking` → "BofA Checking").

### Liabilities

Everything on the Liabilities page is read from the transactions. The
`open` directive may state what the postings cannot, and when it does
it wins over the guess: `rate:` (a yearly percentage, so `rate: 24`
is 24%), `due:` (a day of the month, 1–31), `limit:` (in the display
currency) and `collateral:` (an asset account the loan is secured on,
priced at the month's end).

- **Owed** = the negation of a `Liabilities:*` account's converted
  balance, so a debt is a positive number; a charge raises it and a
  payment lowers it. An account in credit is owed as a negative amount
  and raises an `overpaid` notice.
- **Kind**: `installment` (a loan) or `revolving` (a card). A segment
  of the account name decides when it can — card, credit, visa,
  mastercard, amex, overdraft say card; loan, mortgage, financ, student,
  auto, car, lease say loan — otherwise a debt that took new charges
  after it was opened is a card.
- **Rate** = `rate:` from the open directive, else the interest posted
  against the balance it accrued on, annualised, over the newest six
  interest legs. An interest leg is a
  posting to an `Expenses:` account with "interest" in its name, in a
  transaction that also touches the debt. Interest earned is the same
  reading of `Income:` accounts named that way.
- **Payment** = the median of the last three payments; **due day** =
  `due:` from the open directive, else the median day of the last six,
  once there are two. `next_due` is the due
  day in the current month or the next, while something is owed.
- **Payoff** = months until the balance reaches zero at the current
  payment with interest accruing monthly at `rate / 12` (rounded to the
  cent each month, banker's rounding), plus the interest paid on the
  way; `null` when the payment does not beat the interest. Capped at
  1,200 months. Computed for loans and for cards carrying a balance,
  which are the debts being *paid down*. **Debt-free** = the month the
  last of those is paid off, `null` while none is owed or any of them
  has no payoff; **cost_year** = Σ owed × rate; **blended_rate** =
  cost_year / owed. **assumed_return** = the middle scenario of the
  independence page, so the pay-down-vs-invest reading on a loan uses
  the return that page does.
- **Cycle** (cards): the current month's charges and payments, the
  balance carried past the last payment (charges younger than 31 days
  belong to the cycle being paid into), and whether that payment
  cleared everything older.
- **Notices**: `missed` — a debt paid within the last two months whose
  due day has passed by more than three days this month without a
  payment; `growing` — a card that ended higher three months running
  while carrying a balance; `overpaid` — see above.
- **Upcoming** = the next due payment of every debt within 31 days: the
  usual payment, or everything on a card that is cleared every
  statement.
- **Foreign** = the parts of a debt's balance not in the display
  currency, each converted when it has a price. **Limit** and
  **utilisation** (`owed / limit`) come from `limit:`; **collateral**
  from `collateral:`, with the asset's converted month-end value when
  it can be priced.
- **Beaten** = debts that are over: nothing owed in any currency,
  something once was, and either a loan or a closed account (a card at
  zero that is still open is only between statements). Each keeps its
  peak, principal and interest paid, the first day anything was owed
  and the day of the last payment; most recently beaten first.
- **Cover** = the budget accounts' cash against what the cards hold.
- **Extra** = the spare worked out from the cash, so the reader is
  told it rather than asked for it: `cash` is the cover's cash; `due`
  is Σ upcoming; `spend` is a typical month of cash-paid spending, the
  median over the `basis` months of the `Expenses:` postings in
  transactions that touch a budget `Assets:` account and no
  `Liabilities:` account, over the months that have any; `buffer` is
  a month of the fixed nut (the recurring report's `monthly_fixed`),
  kept back. `now` = cash − due − spend − buffer, floored at zero: the
  lump that could go to the debts this month. `monthly` = the median
  over the same months of income − expenses − the principal paid on
  the debts, floored at zero: what is usually left over once
  everything, the debts included, is paid, and where the "Which
  first?" slider starts.
- **Makeup** (a card carrying a balance): what the carried balance is
  made of, read first in, first out. Every charge opens a lot, split
  across the transaction's other postings that were debited (an
  `Expenses:` account, usually) pro rata, each converted to the card's
  currency at the day's price, and "Other" for whatever cannot be
  attributed; a rise with no such posting is interest when the
  transaction has an interest leg. Interest is spread over the open
  lots pro rata by what each still has left, the last lot taking the
  rounding. A payment or refund pays the oldest lot first, its
  interest before its principal, and closes it when nothing is left.
  Lots are kept per currency. The rows group the open lots by expense
  account, biggest first, each with what it still owes, what was
  charged, the interest it has accrued, the date of its oldest charge
  and the number of charges in it. `null` for loans and for cards
  that owe nothing or carry nothing.
- **Treadmill** (the same cards): the months from `current − basis`
  to the month before the current one, clamped to the first month the
  account moved, each with `charges` (the rises less the interest),
  `payments` and `interest`. The pace is read over the last three
  complete months: `charged` and `paid` are their totals and `net` =
  (paid − charged) / 3 is what the balance really shrinks by a month.
  Its `payoff` is the amortisation of what is owed at that net, or
  `null` when the card is standing still or growing: the honest date,
  against the one the payment alone suggests.
- **Windows**: the interest and payment figures for "the year" cover
  the twelve complete months before the current one, clamped to the
  ledger; a debt's `trail` (its balance after every transaction) covers
  the `basis` months.
- **Desktop presentation**: the liabilities page summarizes what is owed,
  free cash, interest, the next payment, and one section per open debt.
  Each debt shows its recent balance, projected payoff, recent payments,
  and a slider for trying an extra monthly payment. The slider uses the
  same cent-rounded amortization as the core report.

## UI

The native interface is built with `gpui-pre`. `Root` owns the current
ledger reading and the reader's page choices; page-specific models turn core
reports into display-ready figures, and modules under `crates/desktop/src/ui/`
draw them. JetBrains Mono ships in the binary, so the app does not need a
font or network request at startup.

Six tabs share the window: Overview, Budget, Reports, Invest, Debts, and SQL.
Account registers open beneath Budget, while search can jump from any page to
the matching account and month. Digit keys open tabs; `h`/`l` move between
them, `j`/`k` walk rows, `d`/`u` scroll, `/` searches, and `t` switches the
color scheme. Below 640px the same state is drawn in a phone layout.

Night and day palettes live in `crates/desktop/src/theme.rs`. Blue denotes
cash and principal, purple investments, green favorable movement, and red
amounts owed or adverse movement. The embedded JetBrains Mono weights keep
labels, sums, and table columns on the same character grid.

## Performance

- Parse each file once; includes are discovered breadth-first and parsed in
  parallel (`std::thread::scope`) per wave.
- Startup precomputes per-(account, month) currency sums, the transaction
  index, cumulative account balances, and sorted price series.
- Page models are cached by their relevant choices and rebuilt only when the
  ledger, month, basis, or range changes.
- DuckDB is loaded lazily and off the UI thread when the SQL tab is opened.
- Release builds use `lto = "thin"` and one codegen unit.

### Reproducible synthetic benchmark

`bun tools/perf-gen.ts /tmp/big 2100` creates a deterministic 120-month
ledger with 252,505 directives across 123 files. Build the release binary and
open `/tmp/big/main.beancount` while profiling startup, memory, and page
interaction. The generated corpus contains no personal-ledger or host-specific
output.

## Testing

Strict TDD: every behavior lands as a failing test first; `jj commit` after
each red-to-green cycle.

1. **Core unit and integration tests** cover loading, accounting, conversion,
   monthly aggregation, reports, search data, and investment performance.
2. **Desktop model tests** cover every page's display-ready calculations and
   formatting without rendering a window.
3. **Interaction and frame tests** exercise key bindings, scrolling, row
   navigation, SQL editing, reload behavior, and responsive layout in GPUI's
   test harness.
4. **Visual tests** render day, night, desktop, and phone boards and compare
   them with `crates/desktop/tests/reference/`.
5. **Real-ledger tests** are opt-in and keep local financial data out of the
   normal test run and repository.

## Out of scope (for now)

Editing the ledger, institution connections, imports, external reconciliation,
payment execution, editable budget goals, and plugins.
