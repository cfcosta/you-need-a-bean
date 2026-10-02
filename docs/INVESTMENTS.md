# Investments

Open **Invest** in the tab bar or press `4`. The page uses the ledger's first
operating currency and shows holdings as of the app's current day. Future
transactions and future prices are excluded.

The portfolio combines positive balances of non-operating commodities in asset
accounts. It includes small positions and holdings across every account, with
no personal/business split. Operating-currency cash, including balances in
accounts named as investments, is not part of this commodity portfolio.

The allocation chart uses `asset-class` metadata on commodity directives.
Account names, labels, and tickers are searchable in the holdings filter.
Sorting covers value, estimated gain, and name. Filtering does not change the
portfolio totals or allocation denominator. The allocation panel also groups
holdings by account.

Transaction search matches tickers in posting amounts and provides source-file
references.

Unpriced holdings are named separately and excluded from valuation and allocation.
Quotes at least 45 days old are marked as stale. Prices are read from the ledger;
there is no external market feed. For converted prices, the date reflects the
oldest quote in the conversion path used by the ledger.

Unrealized gains compare current value with average acquisition cost per account.
They are estimates, not specific-lot or tax calculations. A holding with missing
acquisition costs has no gain estimate; missing cost is never zero cost. The
coverage figure shows how much priced value has a recorded cost basis. Realized
gains, dividends, fees, and total investment return are not calculated by this
page. Accounting errors withhold gain estimates.

Declare descriptive metadata and prices in the ledger, for example:

```beancount
2026-01-01 commodity FUND
  name: "Broad market fund"
  asset-class: "Equity funds"

2026-09-08 price FUND 125.50 USD
```

The Reports page keeps its compact portfolio summary; the Invest page provides
the detailed view.

## Performance over a period

The performance panel shows opening and closing values, net flows, price gain,
a return estimate, and sampled value history. Dates are end-of-day:
transactions on the opening date are included in opening value, and subsequent
transactions through the current day are period flows. Future entries and
prices are excluded. Samples include the endpoints, month ends, and investment
activity days.

The portfolio boundary is the same non-operating commodities in asset accounts
as the holdings page. Net-zero units transferred between asset accounts are
internal. Purchases, sales, and other net unit movements cross the boundary.
Flow value uses a posting's execution price (`@` or `@@`) when available, then
acquisition cost for additions, otherwise the latest ledger quote on the flow
date. A disposal's old lot cost is never treated as its sale proceeds.

Price gain is closing value minus opening value minus net flows. The percentage
is a **Modified Dietz estimate**, using opening value plus flows weighted by the
fraction of the period remaining after their dates. Flows are assumed to occur
at the end of the day. It is a period percentage, not annualized, and is omitted
when weighted capital is zero or negative. See the [GIPS calculation methodology](https://www.gipsstandards.org/wp-content/uploads/2021/03/calculation_methodology_gs_2011.pdf)
for the flow weighting formula. This implementation does not claim GIPS compliance.

This measures **price performance**, including exchange-rate effects in the
report currency. It excludes cash dividends, interest, fees, and taxes; new units
received as rewards are additions valued at their arrival price, not price gains.
It is not total return. Missing flow conversions or historical valuations prevent
a complete portfolio result. Negative inventory is unsupported. Old quotes are
flagged, and accounting issues withhold gain/return estimates.
Ledger quotes can lag execution prices, so this is only as current as the prices
recorded in the ledger.

The Investments page offers 1M, 3M, 6M, YTD, 1Y, and All windows. YTD opens
on the prior December 31. Other windows use calendar months, clamped to the
last valid day. The chart compares portfolio value with opening value plus net
flows; their difference is price gain. The allocation and holdings sections
remain the current snapshot.
