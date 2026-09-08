"""Regenerate the wholly fictional overview demo for a chosen calendar day."""
import argparse
from datetime import date, timedelta
from decimal import Decimal as D
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("--today", type=date.fromisoformat, default=date.today())
today = parser.parse_args().today
root = Path(__file__).parent
lines = ['; Entirely fictional. Regenerate with examples/build-overview.py.',
         'option "title" "A life in balance"', 'option "operating_currency" "USD"', '']
start = date(today.year - 1, 12, 1)
accounts = [
    ("Assets:Bank:Everyday", "Everyday account", "USD", ['liquidity: "cash"']),
    ("Assets:Bank:Emergency", "A little breathing room", "USD", ['liquidity: "cash"', 'reserve: 10000', 'goal: 24000', f'goal-date: "{today.year + 1}-06-01"']),
    ("Assets:Bank:Travel", "Somewhere by the sea", "USD", ['liquidity: "cash"', 'reserve: 2800', 'goal: 5000', f'goal-date: "{today.year + 1}-03-01"']),
    ("Assets:Investments:Index", "Long-term portfolio", "BEAN", ['liquidity: "investment"']),
    ("Assets:Business:Checking", "Studio account", "USD", ['liquidity: "cash"', 'scope: "business"']),
    ("Liabilities:Loan:Car", "The little blue car", "USD", ['rate: 6', 'due: 20']),
    ("Income:Salary", "Salary", "USD", []),
    ("Income:Business:Consulting", "Studio income", "USD", ['scope: "business"']),
    ("Expenses:Home:Rent", "A place to call home", "USD", []),
    ("Expenses:Food:Groceries", "Good food", "USD", []),
    ("Expenses:Home:Utilities", "Lights & warmth", "USD", []),
    ("Expenses:Subscriptions:Music", "A soundtrack for life", "USD", []),
    ("Expenses:Food:Coffee", "Coffee with friends", "USD", []),
    ("Expenses:Travel", "Places to go", "USD", []),
    ("Expenses:Interest", "Loan interest", "USD", []),
    ("Expenses:Uncategorized", "A receipt to remember", "USD", []),
    ("Equity:Opening", "Opening balances", "USD", []),
]
for account, label, currency, meta in accounts:
    lines += [f'{start} open {account} {currency}', f'  name: "{label}"']
    lines += ['  ' + m for m in meta]
    if account.startswith(("Assets:", "Liabilities:")):
        lines += [f'  updated: "{today}"']
lines += [f'{start} commodity BEAN', '  name: "Broad Market Example Fund"', '  asset-class: "Global equities"', f'{start} price BEAN 100 USD', '']

def txn(day, payee, desc, postings, flag="*", tag=""):
    lines.extend([f'{day} {flag} "{payee}" "{desc}"{tag}'] + [f'  {a}  {v} {c}' for a, v, c in postings] + [''])

def cash(day, payee, expense, amount):
    txn(day, payee, "", [(expense, amount, "USD"), ("Assets:Bank:Everyday", -D(str(amount)), "USD")])

for a, amount in [("Assets:Bank:Everyday", 14000), ("Assets:Bank:Emergency", 18500), ("Assets:Bank:Travel", 2800), ("Assets:Business:Checking", 9200)]:
    txn(start, "", "Opening balance", [(a, amount, "USD"), ("Equity:Opening", -amount, "USD")])
txn(start, "", "Opening investment", [("Assets:Investments:Index", "420 BEAN {100 USD}", ""), ("Equity:Opening", -42000, "USD")])
txn(start, "", "Car financing", [("Assets:Bank:Everyday", 12000, "USD"), ("Liabilities:Loan:Car", -12000, "USD")])
loan = D(12000)
for month in range(1, today.month + 1):
    for day, merchant, account, amount in [
        (1, "Willow House", "Expenses:Home:Rent", 1450),
        (3, "Green Basket", "Expenses:Food:Groceries", 218 + month * 3),
        (8, "Cloud Nine", "Expenses:Subscriptions:Music", 14 if month < 7 else 17),
        (12, "Bright Energy", "Expenses:Home:Utilities", 116),
        (17, "Green Basket", "Expenses:Food:Groceries", 193 + month * 2),
        (22, "Sunday Coffee", "Expenses:Food:Coffee", 54 + month),
    ]:
        d = date(today.year, month, day)
        if d <= today: cash(d, merchant, account, amount)
    salary = date(today.year, month, 5)
    if salary <= today:
        txn(salary, "Northstar Payroll", "Monthly salary", [("Assets:Bank:Everyday", 6200, "USD"), ("Income:Salary", -6200, "USD")])
        txn(salary, "", "Room for tomorrow", [("Assets:Bank:Emergency", 300, "USD"), ("Assets:Bank:Everyday", -300, "USD")])
    payment = date(today.year, month, 20)
    if payment <= today:
        interest = (loan * D(".005")).quantize(D(".01"))
        principal = D(345) - interest
        loan -= principal
        txn(payment, "Bluebird Finance", "Monthly payment", [("Liabilities:Loan:Car", principal, "USD"), ("Expenses:Interest", interest, "USD"), ("Assets:Bank:Everyday", -345, "USD")])
    lines += [f'{today.year}-{month:02}-01 price BEAN {100 + month * 3} USD']
lines += [f'{today} price BEAN {101 + today.month * 3} USD']
cash(today - timedelta(days=2), "Corner Market", "Expenses:Uncategorized", D("78.42"))
# Mark the fictional receipt as an explicit review item.
needle = f'{today - timedelta(days=2)} * "Corner Market"'
lines[:] = [l.replace(needle, needle.replace(' * ', ' ! ')) for l in lines]
for offset in range(0, 4):
    ordinal = today.year * 12 + today.month - 1 + offset
    y, m = divmod(ordinal, 12); m += 1
    salary = date(y, m, 5)
    if today < salary <= today + timedelta(days=90):
        txn(salary, "Northstar Payroll", "Expected salary", [("Assets:Bank:Everyday", 6200, "USD"), ("Income:Salary", -6200, "USD")])
    rent = date(y, m, 1)
    if today < rent <= today + timedelta(days=90): cash(rent, "Willow House", "Expenses:Home:Rent", 1450)
cash(today + timedelta(days=24), "Coastline Rail", "Expenses:Travel", 680)
path = root / "overview.beancount"
path.write_text("\n".join(lines) + "\n")
print(path)
