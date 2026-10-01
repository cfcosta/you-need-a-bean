#!/usr/bin/env python3
"""Writes rough.beancount: a made-up ledger with the rough edges a real one
has and the overview example does not — a BRL operating currency, three
years of history, a brokerage holding several commodities (some sold to
nothing), crypto in long decimals, a credit card paid off, a loan with
no rate, a dozen flagged transactions and months with no income.

Nothing here is anyone's real money. Run it from this directory:

    python3 build-rough.py > rough.beancount
"""

from datetime import date


def months(start, end):
    y, m = start
    while (y, m) <= end:
        yield y, m
        m += 1
        if m == 13:
            y, m = y + 1, 1


out = []
w = out.append
w('option "title" "Rough edges"')
w('option "operating_currency" "BRL"')
w('option "operating_currency" "USD"')
w("")
opens = [
    ("Assets:Bank:Checking", "BRL", 'name: "Checking"\n  liquidity: "cash"'),
    ("Assets:Broker", "", 'name: "Brokerage"\n  liquidity: "investment"'),
    ("Assets:Crypto:Wallet", "", 'name: "Wallet"\n  liquidity: "investment"'),
    ("Liabilities:CreditCard", "BRL", 'name: "Card"\n  rate: 12\n  due: 10'),
    ("Liabilities:Friend", "BRL", 'name: "Friend loan"\n  due: 20'),
    ("Income:Salary", "BRL", 'name: "Salary"'),
    ("Expenses:Home:Rent", "BRL", 'name: "Rent"'),
    ("Expenses:Food:Groceries", "BRL", 'name: "Groceries"'),
    ("Expenses:Shopping", "BRL", 'name: "Shopping"'),
    ("Expenses:Uncategorized", "BRL", 'name: "Uncategorized"'),
    ("Equity:Opening", "BRL", 'name: "Opening"'),
]
for account, cur, meta in opens:
    w(f"2023-10-01 open {account} {cur}".rstrip())
    w(f"  {meta}")
for name, cls in [("AAAA", "etf"), ("BBBB", "stock"), ("CCCC", "stock"), ("DDDD", "stock"), ("BTC", "crypto"), ("ETH", "crypto")]:
    w(f"2023-10-01 commodity {name}")
    w(f'  name: "{name} holding"')
    w(f'  asset-class: "{cls}"')
w("")
w("2023-10-01 * \"Opening balance\"")
w("  Assets:Bank:Checking  20000.00 BRL")
w("  Equity:Opening")
w("")
w("2023-11-02 * \"Broker\" \"Buy AAAA\"")
w("  Assets:Broker  10 AAAA {100.00 BRL}")
w("  Assets:Bank:Checking  -1000.00 BRL")
w("2023-11-02 * \"Broker\" \"Buy BBBB\"")
w("  Assets:Broker  5 BBBB {200.00 BRL}")
w("  Assets:Bank:Checking  -1000.00 BRL")
w("2023-11-02 * \"Broker\" \"Buy CCCC\"")
w("  Assets:Broker  3 CCCC {50.00 BRL}")
w("  Assets:Bank:Checking  -150.00 BRL")
w("2023-11-02 * \"Broker\" \"Buy DDDD\"")
w("  Assets:Broker  2 DDDD {40.00 BRL}")
w("  Assets:Bank:Checking  -80.00 BRL")
w("2025-01-15 * \"Broker\" \"Sell CCCC\"")
w("  Assets:Broker  -3 CCCC {50.00 BRL} @ 60.00 BRL")
w("  Assets:Bank:Checking  180.00 BRL")
w("  Income:Salary  -30.00 BRL")
w("2025-01-15 * \"Broker\" \"Sell DDDD\"")
w("  Assets:Broker  -2 DDDD {40.00 BRL} @ 40.00 BRL")
w("  Assets:Bank:Checking  80.00 BRL")
w("2024-02-01 * \"Exchange\" \"Buy BTC\"")
w("  Assets:Crypto:Wallet  0.00011627 BTC {300000.00 BRL}")
w("  Assets:Bank:Checking  -34.88 BRL")
w("2024-02-01 * \"Exchange\" \"Buy ETH\"")
w("  Assets:Crypto:Wallet  0.005537110808728093 ETH {14000.00 BRL}")
w("  Assets:Bank:Checking  -77.52 BRL")
w("")

no_income = {(2025, 3), (2025, 4), (2025, 5), (2025, 6)}
flagged = 0
for i, (y, m) in enumerate(months((2023, 10), (2026, 9))):
    if (y, m) not in no_income:
        w(f'{y}-{m:02d}-05 * "Employer" "Salary"')
        w("  Assets:Bank:Checking  10000.00 BRL")
        w("  Income:Salary")
    w(f'{y}-{m:02d}-01 * "Landlord" "Rent"')
    w("  Expenses:Home:Rent  3500.00 BRL")
    w("  Assets:Bank:Checking")
    food = 1200 + (i % 5) * 37
    w(f'{y}-{m:02d}-09 * "Market" "Groceries"')
    w(f"  Expenses:Food:Groceries  {food}.00 BRL")
    w("  Assets:Bank:Checking")
    w(f'{y}-{m:02d}-12 * "Store" "Things"')
    w("  Expenses:Shopping  800.00 BRL")
    w("  Liabilities:CreditCard")
    w(f'{y}-{m:02d}-20 * "Card" "Pay the card"')
    w("  Liabilities:CreditCard  800.00 BRL")
    w("  Assets:Bank:Checking")
    if i % 3 == 0 and flagged < 12:
        flagged += 1
        w(f'{y}-{m:02d}-14 ! "A shop with a rather long name, ltda." "Receipt"')
        w(f"  Expenses:Uncategorized  {40 + flagged}.00 BRL")
        w("  Assets:Bank:Checking")
    for name, base, step in [("AAAA", 100, 3), ("BBBB", 200, -2), ("BTC", 300000, 4000), ("ETH", 14000, 100)]:
        w(f"{y}-{m:02d}-28 price {name} {base + step * i:.2f} BRL")
    w("")

w('2026-06-01 * "Friend" "Borrowed"')
w("  Assets:Bank:Checking  9000.00 BRL")
w("  Liabilities:Friend")
for m in (7, 8, 9):
    w(f'2026-{m:02d}-20 * "Friend" "Paid back"')
    w("  Liabilities:Friend  1666.67 BRL")
    w("  Assets:Bank:Checking")
w('2026-10-20 * "Friend" "Paid back"')
w("  Liabilities:Friend  2500.00 BRL")
w("  Assets:Bank:Checking")

print("\n".join(out))
