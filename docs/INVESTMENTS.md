# Investments

Open **Investments** in the sidebar, or go directly to `/investments`. The page
uses the ledger's first operating currency and shows holdings as of the reader's
local calendar day. Future transactions and future prices are excluded.

The portfolio combines positive balances of non-operating commodities in asset
accounts. It includes small positions and holdings across every account, with
no personal/business split. Operating-currency cash, including balances in
accounts named as investments, is not part of this commodity portfolio.

The allocation chart uses `asset-class` metadata on commodity directives.
Click a class to filter the holdings; account names, labels, and tickers are
searchable. Sorting covers value, estimated gain, name, and oldest quote.
Filtering does not change the portfolio totals or allocation denominator.

Expand a holding to inspect its quantity, price date, activity dates, and the
accounts that hold it. Account links open their registers. Transaction search
matches tickers in posting amounts and provides source-file references.

Unpriced holdings are named separately and excluded from valuation and allocation.
Quotes at least 45 days old are marked as stale. Prices are read from the ledger;
there is no external market feed. For converted prices, the date reflects the
oldest quote in the conversion path used by the ledger.

Unrealized gains compare current value with average acquisition cost per account.
They are estimates, not specific-lot or tax calculations. A holding with missing
acquisition costs has no gain estimate; missing cost is never zero cost. The
coverage figure shows how much priced value has a recorded cost basis. Realized
gains, dividends, fees, and total investment return are not calculated by this
page. Accounting errors and connection failures withhold gain estimates.

Declare descriptive metadata and prices in the ledger, for example:

```beancount
2026-01-01 commodity FUND
  name: "Broad market fund"
  asset-class: "Equity funds"

2026-09-08 price FUND 125.50 USD
```

The existing Reports page keeps its compact portfolio summary and links here
for the detailed view.
