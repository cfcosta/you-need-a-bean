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
- [x] **5 · Tags and links as projects** — `#renovation` and
      `^trip-japan` are how beancount users scope work across months and
      categories, and the UI aggregates them nowhere. Total, count and
      date span per topic.
      The mock fixture demonstrates both shapes that are not projects:
      two unique importer payment links appear on one transaction each,
      while one repeated transfer tag moves no money. Two other names
      span several transactions and remain reportable topics. So the
      card doesn't guess from a name; it reports transactions, months
      and categories crossed, then counts what it excluded.
      Refunds and reimbursements net out, because a trip the employer
      paid half of did not cost what it charged.
- [x] **6 · Income breakdown** — income collapses to one number today.
      Split by `Income:*` source, flag source concentration, and show
      passive income as a share of spend: the Coast/Barista-FIRE bar.
      Concentration reads better as effective sources — `1 / Σ share²`
      — than as the top share: ten names that pay out 56/43/1 is two
      sources wearing ten labels, and the number says 2.03.
      What counts as passive is the ledger's call first: `income:
      passive` or `income: active` on the `open` directive settles it,
      and the account name is only consulted when the ledger is silent.
      The view reports how many sources were declared and how many were
      guessed, so the card never presents a heuristic as a fact.
      Cashback and referral commission are deliberately not passive — a
      rebate stops the month the spending does.
- [x] **7 · What's missing from these numbers** — commodities with no
      price (silently excluded from net worth), stale price directives,
      `!`-flagged transactions, uncategorized spend, loader warnings.
      Every number on this page is one conversion away from wrong.
      Unpriced commodities were already named, on the page and in the
      growth card, because task 2 could not be honest without them: a
      sale of something unpriced reads as a market gain nothing caused,
      so the implied return is withheld while any price is missing.
      The rest now sits in one card, where each row is a count and an
      amount — the count says how much work is outstanding, the amount
      says how much of the page is standing on it. Nothing priced is
      drawn louder than priced-long-ago, because the first is money
      missing from the totals and the second is money present at a
      price that old, which is worse: it still looks like a number.
      Staleness is measured against today rather than the ledger, six
      weeks being long enough that a monthly price never trips it, and
      only against commodities still held — a three-year-old price on
      a closed position holds nothing up. Dust is dropped for the same
      reason. `Expenses:Misc` is not a gap: miscellaneous is a
      decision, and only the segments that mean "I haven't decided
      yet" count as uncategorized.
- [x] **8 · FIRE variants** — Coast FIRE (stop saving today, still get
      there in N years), lean FIRE off fixed costs, and what saving more
      per month buys. All three have shipped: task 3 prices the fixed
      costs the lean target and the lean runway are built on.
- [x] **9 · Net worth composition** — stack the area chart by cash /
      investments / liabilities so it shows *what* is growing, not just
      that something is.
- [x] **10 · Seasonality** — median spend per calendar month across
      years, and the year-end projection that shape implies.
      A trailing average calls December a surprise every December. The
      median across years calls it December, and dividing each month by
      the twelve leaves the shape with the level taken out — which is
      what makes the projection work: the year ahead is priced at what
      this year has been paying for the months behind it, so the level
      cancels and only the shape carries. The sample is capped at five
      prior years, and the cap is not a detail. A synthetic long-history
      fixture demonstrates that distant price levels distort the current
      pace. Five recent complete years provide enough samples for a
      median without mixing in spending levels from a different era.
      The year being
      projected is never its own baseline, and a ledger that cannot
      cover twelve calendar months even once reports no shape at all
      rather than eight months that appear to cost nothing.
- [x] **11 · Payee leaderboard** — top merchants by trailing-year spend,
      with frequency and average ticket.
      Frequency and ticket size are the point, not decoration. The year
      card says the money went to groceries; it does not say it went to
      one supermarket forty times. Twelve identical small charges are a
      subscription and one line cancels them; a hundred medium ones are
      a habit and nothing cancels them at all. Names are taken as the
      ledger writes them, trimmed and no further — deciding that two
      spellings are one merchant is a guess, and a ranking built on
      guesses ranks the guesses. Transfers are excluded however they
      are labelled, and a purchase returned in full ranks nowhere,
      since neither is a charge. The synthetic payee fixture deliberately
      mixes named and unnamed spending, and includes a name spanning
      multiple categories, so both limitations remain visible. The
      unnamed share is stated in the headline, while the categories and
      months behind each name remain available on hover.

## Conventions these follow

- Money is quantized to cents at the leaves, so displayed sums add up.
- Conversion happens at the date of the thing being valued: month end
  for balances, the transaction's own date for flows.
- Windows are trailing and exclude the anchor month, which is usually
  still in progress. `Ledger::window` is the one definition.
- Amounts that can't be converted are excluded from totals and reported
  by task 7 rather than silently dropped.
