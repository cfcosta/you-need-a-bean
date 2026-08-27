# reports — what's there and what's coming

The reports page answers the questions the monthly budget can't: not "did
I overspend on groceries" but "where is this going, and when do I get to
stop". Everything here is a pure read over `Ledger`, computed in
`crates/core/src/reports/` and drawn by `ui/src/components/Reports.tsx`.

## Shipped

- **FIRE / 4% rule** — 25× trailing annual spend, progress against net
  worth, time-to-target at 3/5/7% real returns.
- **Net worth** — assets + liabilities at month end, whole ledger range.
- **Monthly cashflow** — income − expenses per month.
- **Savings rate** — share of income kept, with a 3-month trend line.
- **Where the year went** — trailing-twelve-month spend per expense group.

## Planned

Each line is one task: core computation (with tests) → API shape → card.

- [x] **1 · Runway** — liquid assets ÷ monthly spend. FIRE answers "when
      can I stop?"; nothing yet answers "what if income stops next
      month?". Liquid = `AccountKind::Budget` assets, which already
      excludes stock-lot accounts. A second figure runs it against fixed
      costs only ("14 months if you cut discretionary"), once task 3
      knows what fixed costs are.
- [x] **2 · Contributions vs market** — beancount's balancing identity
      makes `Δ(assets+liabilities) − (income − expenses) − equity` the
      part of net worth growth that wasn't you saving. Splits each net
      worth move into "you added X · markets added Y", with an implied
      return for the year.
- [x] **3 · Recurring charges** — cluster transactions by (account,
      payee) on a regular cadence with a stable amount. Yields the fixed
      monthly nut, price increases ("Netflix 15.99 → 17.99, +$24/yr"),
      and lapsed subscriptions still charging. Feeds tasks 1 and 8.
      Detection is deliberately strict — three charges, a cadence two
      thirds of the gaps agree on, and amounts within 25% of their
      median — so what it finds is a floor, never the whole bill. The
      view carries `coverage` (fixed over total spend) for exactly that
      reason, and the page never shows a lean figure without it: sparse
      coverage is only a lower bound, while broader coverage makes the
      estimate more representative. A price rise has to clear 2% of the
      prior charge to count, so a subscription
      billed in another currency does not report a raise every time the
      exchange rate moves.
- [x] **4 · Year over year + biggest movers** — the year card ranks but
      never compares. Add the prior twelve months as a delta, plus the
      categories whose trailing quarter moved most against the quarter
      before it: the direct answer to "why is my spending up?".
      Both comparisons refuse to clamp: a window that the ledger cannot
      cover in full comes back empty rather than measuring a year
      against the four months that happen to precede it, which reports
      a collapse that is only the edge of the data. Movers rank by
      money and not by percentage — a category doubling from 5 to 10
      answers nothing — and a move must clear 1% of the bigger quarter
      to earn a row, a floor that scales with the ledger instead of
      assuming a currency.
- [ ] **5 · Tags and links as projects** — `#renovation` and
      `^trip-japan` are how beancount users scope work across months and
      categories, and the UI aggregates them nowhere. Total, count and
      date span per topic.
- [ ] **6 · Income breakdown** — income collapses to one number today.
      Split by `Income:*` source, flag source concentration, and show
      passive income as a share of spend: the Coast/Barista-FIRE bar.
- [~] **7 · What's missing from these numbers** — commodities with no
      price (silently excluded from net worth), stale price directives,
      `!`-flagged transactions, uncategorized spend, loader warnings.
      Every number on this page is one conversion away from wrong.
      Unpriced commodities are already named, on the page and in the
      growth card, because task 2 could not be honest without them: a
      sale of something unpriced reads as a market gain nothing caused,
      so the implied return is withheld while any price is missing. The
      rest of the list is still open.
- [x] **8 · FIRE variants** — Coast FIRE (stop saving today, still get
      there in N years), lean FIRE off fixed costs, and what saving more
      per month buys. All three have shipped: task 3 prices the fixed
      costs the lean target and the lean runway are built on.
- [x] **9 · Net worth composition** — stack the area chart by cash /
      investments / liabilities so it shows *what* is growing, not just
      that something is.
- [ ] **10 · Seasonality** — median spend per calendar month across
      years, and the year-end projection that shape implies.
- [ ] **11 · Payee leaderboard** — top merchants by trailing-year spend,
      with frequency and average ticket.

## Conventions these follow

- Money is quantized to cents at the leaves, so displayed sums add up.
- Conversion happens at the date of the thing being valued: month end
  for balances, the transaction's own date for flows.
- Windows are trailing and exclude the anchor month, which is usually
  still in progress. `Ledger::window` is the one definition.
- Amounts that can't be converted are excluded from totals and reported
  by task 7 rather than silently dropped.
