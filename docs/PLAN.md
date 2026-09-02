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
transaction list with expandable metadata. The look has since moved on
from the mockup's Tokyo Night palette to the "Roast" and "Paper" schemes
described under UI below.

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
- Anything else under `/api/` → 404 JSON; bad month/currency → 400.
- `/` and static assets → embedded UI (SPA fallback to index.html).

## UI

Bun + React 19 + Tailwind v4 (`bun-plugin-tailwind`). The design is an
editorial finance journal in two schemes, both sets of custom properties
in `ui/src/index.css`: "Roast" (dark, the default) and "Paper" (light).
Gold is money that is yours, copper is what the market holds for you,
sage is kept or under, coral is over or owed. Figures and titles are set
in Fraunces, the interface in Inter, account names in JetBrains Mono; the
fonts live under `ui/src/fonts/` and are inlined into the CSS at build
time, so the app never reaches for the network. The scheme follows the
system by default and can be pinned from the switch at the foot of the
sidebar; that choice is kept in local storage (`ui/src/theme.ts`), not in
the URL, since it is about the screen rather than the ledger. Components
map 1:1 to the mockup's renderers: `Sidebar`, `Topbar`, `StatStrip`,
`BudgetTable` (VsBar), `Inspector` (TargetCard, HistoryChart,
CurrencySplit, TxnList). Client state: `{month, basis, cur, cat,
openTxns, closedGroups}` — same as the mockup's `state` object.

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

## Out of scope (for now)

Live reload on file change, URL hash state, editing anything, documents,
budget goal editing (targets are always trailing averages), plugins.
