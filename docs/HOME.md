# The financial home

The overview is the landing page. It brings a dated financial position,
known future commitments, savings targets, source coverage, and a review
queue together. Budget, reports, and liabilities remain available.

Start with the entirely fictional demonstration:

```sh
cargo run -- examples/overview.beancount
# Move the demo's dates forward when needed:
python3 examples/build-overview.py --today 2026-09-07
```

The home offers Personal, Business, and Everything scopes. Scope changes
apply to the home; the existing reports and sidebar still cover the full
ledger. The independence scenario within Reports uses personal funds and
personal spending. Opening a particular account shows its own register.

## Dated facts and planning

Every browser request sends its local calendar date as `as_of=YYYY-MM-DD`.
API callers can use the same parameter for a reproducible reading; without
it, the server uses its UTC calendar day. Current balances and prices stop
at that day. Future ledger entries remain searchable and feed the forecast.

Cash after reserves is the sum of eligible cash accounts less declared
reserves. It is **not a safe-to-spend guarantee**. Known upcoming outflows
appear separately. Unrecorded obligations, source gaps, or incomplete prices
can still change the picture. Negative available cash remains negative.

The 30/60/90-day timeline combines recorded future cash movements with
estimated recurring cash expenses and debt payments. Internal transfers
between eligible cash accounts do not become income or spending. Recorded
bills replace matching estimates. Cash-paid recurring expenses are included;
card purchases are represented by estimated debt payments to avoid counting
the purchase and its payment twice. Installment estimates stop after the
remaining balance plus modeled monthly interest is paid. Recurring card
payments assume continued use; future statements can differ.

Unrecorded future income and other spending are excluded. The slider adds a
constant monthly spending scenario, accrued daily on a 30-day basis. It does
not edit the ledger or save an allocation. Foreign future flows use the
latest price available at the snapshot date, not future quotes.

## Account metadata

Metadata belongs beneath an existing `open` directive. Amounts for reserves
and goals use the ledger's **first operating currency**, independently of the
currency selected for display.

```beancount
2026-01-01 open Assets:Bank:Savings USD
  name: "A little breathing room"
  scope: "personal"
  liquidity: "cash"
  reserve: 3000
  goal: 10000
  goal-date: "2027-06-01"
  updated: "2026-09-07"
```

- `scope`: `personal` or `business`. Without it, a `Business` account segment
  selects business; other accounts default to personal. Declare business
  income and expenses explicitly if their names do not identify them.
- `liquidity`: `cash`, `investment`, `illiquid`, `receivable`, or `restricted`.
  This is separate from `ynab` sidebar metadata. Recognizable property,
  receivable, and retirement account names receive conservative defaults;
  otherwise the existing budget/tracking distinction supplies the fallback.
  Review inferred classifications. The home can recognize explicitly
  declared cash even in a tracking account.
- `reserve`: money excluded from available cash; it does not move funds.
- `goal`: target for this account. Account value measures progress; a target
  alone does not reserve money or create a monthly budget allocation.
- `goal-date`: optional target date, written as a quoted ISO date.
- `updated`: the source coverage date you verified, written as a quoted ISO
  date. It is not inferred from recent transaction activity.

Coverage also shows balance-assertion evidence. Recent means within seven
days. A passing assertion checks the ledger against a declared balance;
it does not independently verify a bank statement or establish that every
institution has been included. The UI keeps last activity separate.

## Accounting contract

Parsing and validation are separate. The model checks balance assertions
at the start of their date, including child accounts and explicit tolerance.
It checks transaction weights and reports ambiguous omitted amounts.
Source locations travel into the review queue and transaction search.

Padding, plugins, and unresolved empty-cost lot booking are explicitly
reported as unsupported transformations. They are **not executed**. Expand
those transformations through a compatible external Beancount workflow
before relying on the resulting totals. This is not full Beancount engine
parity. Invalid or unsupported accounting remains inspectable, but the home
withholds available-money and forecast figures, and global retirement/debt
planning pauses. Missing valuation prices also pause those plans. A failed
reload retains the last readable ledger with a persistent warning.

Unconfirmed transactions, uncategorized spending, stale quotes, and source
coverage gaps remain visible review items. They are not automatically fixed.
The older realistic fixture now exposes two balance mismatches that were
previously ignored; it remains useful for reviewing incomplete-data states.

## Investigation and API

Use Search or Cmd/Ctrl+K to search accounts, payees, dates, narration, tags,
links, and metadata. All words must match, case-insensitively. Results include
future entries, full postings, attached documents, and source file/line.
Search spans the whole ledger and returns 50 results per page.

- `GET /api/home?scope=personal&cur=USD&as_of=2026-09-07`
- `GET /api/search?q=rent&offset=0&cur=USD&as_of=2026-09-07`
- `/api/summary` now includes structured accounting issues and assertions.
- `/api/reports` and `/api/liabilities` expose `planning_ready`.

The app stays local and read-only. Institution connections, imports,
external reconciliation, editor changes, and payment execution remain outside
it. Changes saved to the ledger are picked up automatically.
