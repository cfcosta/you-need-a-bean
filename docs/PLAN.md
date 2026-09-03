# you need a bean — implementation plan

A read-only, YNAB-style monthly budget viewer for beancount ledgers.
Single Rust binary, UI embedded. You point it at a ledger file and it
serves a local web app:

```
you-need-a-bean examples/example.beancount
# → serving http://127.0.0.1:2326  (parsed 2,412 directives across 1 file in 38ms)
```

The layout follows `docs/mockup-v3.html`: dark sidebar with ledger chip
and account balances, monthly budget table with "vs typical" bullet bars,
right inspector with target card / 6-month chart / currency split /
transaction list with expandable metadata. It keeps the mockup's Tokyo
Night palette; the typography and the shape of the cards have moved on,
as described under UI below.

## Architecture

```
crates/core   you-need-a-bean-core   pure library: load → model → queries
crates/cli    you-need-a-bean-cli    binary "you-need-a-bean": args, axum server, embedded UI
ui/           Bun + React + Tailwind SPA, built to ui/dist, embedded via rust-embed
```

Data flow: parse all files once at startup into an immutable `Ledger`
(indexed aggregates), share it behind an `Arc`, answer every API request
from the in-memory model. No database, no re-parsing per request (a file
watcher can come later).

- **Parser crate**: `beancount-parser` (nom-based, fast, metadata/tags/links
  support, generic over the decimal type). We use `rust_decimal::Decimal`
  internally; amounts serialize as JSON numbers at the API boundary.
- **HTTP**: axum + tokio. **Embedding**: rust-embed over `ui/dist`;
  `--ui-dir` flag overrides with a filesystem path for UI development.
- **Build**: `crates/cli/build.rs` runs `bun install` + `bun run build` in
  `ui/` when `ui/dist` is stale, so `cargo build` always produces a
  self-contained binary (matches the flake's `you-need-a-bean-cli` package).

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
  Empty window → no target (UI shows "—", no percentage).
- **Status**: ratio r = spent/avg; `over` if r > 1, `warn` if r > 0.85,
  else `good` (mockup thresholds).
- **Conversion** to the display currency at date D (end of selected month):
  direct price ≤ D, else inverse price ≤ D, else one-hop pivot through an
  operating currency. Unconvertible amounts are excluded from converted
  totals but always shown in the native split (e.g. `VACHR`).
- **Income tile** = −(sum of postings to `Income:*`) for the month, converted.
- **Sidebar balances** = cumulative postings from ledger start through the
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
- **In the UI** (`ui/src/debt.ts`): the "Which first?" card simulates
  the debts being paid down, month by month, in two orders —
  highest rate first (avalanche) and smallest balance first
  (snowball) — with the extra the reader picks and every finished
  debt's payment rolled into the next; the masthead's debt-free
  sentence uses the same rollover. The two orders are drawn as a
  race: one bar per debt from now to the month it ends, a finish
  line per order. Each card leads with a sentence that reads its
  figures (`loanLede`, `cardLede`, `securedText`, `foreignText`,
  `investVerdict`, `beatenText`), with the figures in bold.
- **No subtitles.** Every owing debt gets a colour (`hues`, five
  hues cycling, biggest debt first) that it keeps everywhere: the
  masthead's owed-versus-cost bars (`costShares`), the strip of the
  next 31 days with each payment standing where it falls
  (`stripMarks` lays labels out in lanes so they never collide), the
  ring on its card, and its bar in the race. A card's head says what
  the debt is with a ring (progress for a loan, limit used for a
  card) and pills — the rate, toned by `rateTone` against the assumed
  return; secured on what; carrying, cleared, or in credit; billed in
  which currency — instead of a line of text under the title. Beaten
  debts are tiles, not rows.
- **This month** (`PlanCard`): the cash in budget accounts drawn as a
  bar cut into what is due in the next 31 days, a typical month of
  spending, the fixed costs kept back, and what is spare, with a
  sentence that reads the sum (`roomText`) and, when the spare covers
  the carried card balances, how many months of fixed costs would
  still be in hand. Under it, a row per payment to send (`monthPlan`):
  the day, the debt in its colour, the amount and what is left on the
  debt after it. The rows start from `upcoming`; the spare is then
  placed down the chosen order (`targets`: the debts being paid down,
  skipping a loan whose rate is under the assumed return, since on
  paper that money does better invested), each debt taking what
  clears it before the next gets any, and a debt whose payment already
  went this month gets a row dated today. A toggle picks the order
  when more than one debt is in line; what is left over is said in a
  sentence (`leftText`). "Add the due dates to your calendar" builds
  an iCalendar file (`ui/src/ics.ts`, `dueEvents`): one all-day event
  per upcoming payment, repeating monthly on the due day for the
  payoff's months (twelve for a card cleared in full, whose amount
  varies, so its summary says "statement due"), a stable UID per
  account so a re-import updates rather than duplicates, lines folded
  at 75 octets and text escaped as RFC 5545 asks. A due day past a
  month's end skips that month, as the standard has it.
- **Which first?** starts its slider at the usual monthly surplus
  (`sliderStart`, snapped to the slider's step) and says so
  (`monthlyText`), so the race opens on what the reader can actually
  sustain rather than on zero.
- **On a carried card**: "What is carried" is the makeup as a bar in
  the card's colour, fading from the biggest part to the smallest, a
  row per part (what it was for, how many charges since when, what is
  owed and the interest it has run up) and a sentence naming the rate
  being paid on what (`makeupText`). "On the treadmill" is a pair of
  bars per month, what went on the card against what came off it,
  interest stacked on the charges in red, the months the pace is read
  over drawn in full and the earlier ones faded, then the sentence
  with the real monthly change, the honest payoff month against the
  one the payment alone suggests, and the interest on the way
  (`treadmillText`).

## HTTP API

All responses are JSON. Amounts are numbers in the requested display
currency unless stated. `cur` defaults to the first operating currency,
`basis` to 6 (allowed: 3, 6, 12).

- `GET /api/summary`
  `{ title, files, directives, parse_ms, operating_currencies, months:
  [first, …, last], today, default_month }`
- `GET /api/month/{YYYY-MM}?basis=&cur=`
  `{ month, is_current, day, days_in_month, income, spent, typical,
  groups: [{ name, spent, avg, categories: [{ account, label, spent, avg,
  status, ratio, split: {CUR: amount} }] }], accounts: { budget: [...],
  tracking: [...] } }` where each account row is `{ account, label,
  balances: {CUR: amount}, converted }`.
- `GET /api/category/{account}/{YYYY-MM}?basis=&cur=`
  `{ account, label, spent, avg, status, window: [from, to], history:
  [{month, spent}×6], split, txns: [{ date, flag, payee, narration, tags,
  links, meta, amount, currency, converted, postings: [{account, amount,
  currency}] }] }`
- `GET /api/liabilities?basis=&cur=`
  `{ month, owed, installment, revolving, interest: { month, year,
  earned_month, earned_year, window: [from, to] }, cost_year,
  blended_rate, debt_free, assumed_return, cover: { cash, owed, covered,
  after }, upcoming: [{ account, label, date, amount }], notices: [{
  kind, account, label, amount, day }], debts: [{ account, label, kind,
  owed, balances: {CUR: amount}, foreign: [{ code, amount, converted }],
  limit, utilisation, collateral: { account, label, value } | null,
  peak, progress, rate, payment, due_day, next_due, principal_paid,
  interest_paid, payments: [{ date, total, principal, interest }],
  history: [{ month, owed }], trail: [{ date, owed, delta }], payoff: {
  months, month, interest }, cycle: { charges, payments, carried,
  in_full }, treadmill: { months: [{ month, charges, payments,
  interest }], pace, charged, paid, net, payoff } | null, makeup: {
  rows: [{ account, label, owed, charged, interest, since, count }],
  total } | null }], beaten: [{ account, label, peak, principal_paid,
  interest_paid, first, last }], extra: { cash, due, spend, buffer,
  now, monthly }, unpriced }` — rules under "Liabilities" above.
  Zero is always sent as `0.0`, never `-0.0`.
- Anything else under `/api/` → 404 JSON; bad month/currency → 400.
- `/` and static assets → embedded UI (SPA fallback to index.html).

## UI

Bun + React 19 + Tailwind v4 (`bun-plugin-tailwind`). The design is an
editorial finance journal in the Tokyo Night colours, two sets of custom
properties in `ui/src/index.css`: Night (dark, the default) and Day
(light). Blue is money that is yours, purple is what the market holds
for you, green is kept or under, red is over or owed. Figures and titles
are set in Bricolage Grotesque, the interface in Inter, account names in
JetBrains Mono; the fonts live under `ui/src/fonts/` and ship inside
the bundle, so the app never reaches for the network. The scheme
follows the system by default and can be pinned from the switch at the
foot of the sidebar; that choice is kept in local storage
(`ui/src/theme.ts`), not in the URL, since it is about the screen rather
than the ledger. Components map 1:1 to the mockup's renderers: `Sidebar`, `Topbar`, `StatStrip`,
`BudgetTable` (VsBar), `Inspector` (TargetCard, HistoryChart,
CurrencySplit, TxnList). Client state: `{month, basis, cur, cat,
openTxns, closedGroups}` — same as the mockup's `state` object.

Three pages share the shell: the budget, `Reports` (`/reports`) and
`Liabilities` (`/liabilities`), routed in `ui/src/router.ts`. The
Liabilities page is one masthead (owed, the month's and the year's
interest, a year at these rates, the debt-free month, cash against the
cards, interest earned, a bar cut per debt, the payments coming up),
the notices, then a card per debt: a loan's balance by month with the
projection to zero and a slider that tries a bigger payment
(`ui/src/debt.ts` amortises the way the server does, to the cent), the
principal against the interest paid, and the recent payments; a card's
day-by-day sawtooth with every payment marked and its cycle. Card
heads, keys and chart scales are shared through `components/Card.tsx`
and `chart.ts`.

## Performance

- Parse each file once; includes discovered breadth-first and parsed in
  parallel (`std::thread::scope`) per wave.
- Startup precomputes: per-(account, month) currency sums + transaction
  index; per-account cumulative balances per month; sorted price series
  per currency pair. Requests are hash lookups + a few dozen adds.
- Release profile: `lto = "thin"`, `codegen-units = 1`.
- Acceptance: the 339 KB `examples/example.beancount` parses well under
  100 ms; a synthetic ~100k-directive ledger stays comfortably subsecond
  end-to-end at startup, and API responses stay in single-digit ms.

### Reproducible synthetic benchmark

Corpus: `bun tools/perf-gen.ts /tmp/big 2100` — a deterministic
120-month ledger (2016-09 – 2026-08), 252,505 directives across 123
files (~26 MB), two operating currencies, and glob includes. Every
account, payee, amount, and date in the corpus is generated.

Build the release binary, serve `/tmp/big/main.beancount`, then measure
startup through the first HTTP response, resident memory, median/max API
latency, static-asset latency, browser completion, and rapid month
navigation. Results depend on the host and should be recorded locally;
the committed corpus contains no personal-ledger or host-specific output.

## Testing

Strict TDD: every behavior lands as a failing test first; `jj commit`
after each red→green cycle (Conventional Commits).

1. **core unit/integration tests** — include resolution (relative, glob,
   dedup, cycles), options, pretty names, monthly aggregation, averages,
   status, conversion incl. pivot + unconvertible, balances, budget vs
   tracking split, transaction listings. Fixture ledgers live in
   `crates/core/tests/fixtures/`.
2. **API tests** — axum router exercised in-process (`tower::ServiceExt`),
   asserting JSON shapes against a fixture ledger.
3. **Corpus test** — every root ledger in `examples/` (including the new
   `examples/multi/` include-based, multi-currency one) must load with a
   plausible directive count and serve `/api/summary` + a month page.
4. **E2E** — headless Chromium driven over CDP against the release binary
   on `examples/example.beancount`: sidebar chip, friendly names, month
   navigation, currency toggle, inspector interactions, screenshots.
5. **UI unit tests** — `bun test` over the pure modules (`router`,
   `months`, `format`, `debt`); the amortisation fixture there matches
   the core one figure for figure.

## Out of scope (for now)

Live reload on file change, URL hash state, editing anything, documents,
budget goal editing (targets are always trailing averages), plugins.
