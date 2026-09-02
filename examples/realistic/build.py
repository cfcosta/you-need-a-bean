#!/usr/bin/env python3
"""Rebuilds this directory: a synthetic ledger with the shape of a real one.

    python3 examples/realistic/build.py

Deterministic — same seed, same bytes — so re-running it on an unchanged
script is a no-op and the diff stays reviewable. Nothing here is taken from
anybody's actual ledger: every merchant, client, account and amount is
invented.

The ledger is anchored to a date, because half the reports measure trailing
windows against today and a ledger that stops a year ago has nothing for
them to find. To move it forward, set `TODAY` below to roughly the current
date and re-run. `START` must stay at least 24 months before it: the year
and movers cards compare a window against the one before it and report
nothing at all rather than measure a year against however much history
happens to precede it.

Standard library only. No beancount, no dependencies.
"""

import glob
import os
import random
import shutil
from collections import defaultdict
from datetime import date, timedelta
from decimal import Decimal, ROUND_HALF_UP

SEED = 0x5EED_B00C
START = date(2024, 1, 1)
TODAY = date(2026, 9, 2)
LAST = date(2026, 9, 1)           # the current month is deliberately partial

rng = random.Random(SEED)
OUT = os.path.dirname(os.path.abspath(__file__))


def d(x) -> Decimal:
    return Decimal(str(x)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)


def units(x, places=4) -> Decimal:
    q = Decimal(1).scaleb(-places)
    return Decimal(str(x)).quantize(q, rounding=ROUND_HALF_UP)


def money(rng, lo, hi) -> Decimal:
    return d(rng.uniform(lo, hi))


# --------------------------------------------------------------------------
# The ledger accumulator
# --------------------------------------------------------------------------

class Book:
    def __init__(self):
        self.entries = []                      # (date, sort, text)
        self.balances = defaultdict(Decimal)   # (account, currency) -> units
        self.lows = {}                         # the worst it ever got
        self.flows = defaultdict(lambda: [Decimal(0), Decimal(0)])

    def add(self, when, text, sort=1):
        self.entries.append((when, sort, text))

    def post(self, account, amount, currency):
        self.balances[(account, currency)] += Decimal(amount)
        if currency == "USD":
            self.flows[account][0 if amount >= 0 else 1] += Decimal(amount)
        if currency == "USD":
            key = (account, currency)
            low = self.lows.get(key)
            here = self.balances[key]
            if low is None or here < low:
                self.lows[key] = here

    def balance(self, account, currency="USD"):
        return self.balances[(account, currency)]


BOOK = Book()
STATE = {}


def txn(when, flag, payee, narration, postings, tags=(), links=(), meta=()):
    """One transaction. `postings` is a list of dicts; exactly one may elide
    its amount, and it absorbs the residual the way beancount would."""
    head = f"{when} {flag}"
    if payee is not None:
        head += f' "{payee}"'
    head += f' "{narration}"' if narration is not None else ' ""'
    for t in tags:
        head += f" #{t}"
    for l in links:
        head += f" ^{l}"
    lines = [head]
    for key, value in meta:
        lines.append(f'  {key}: "{value}"')

    residual = defaultdict(Decimal)
    elided = None
    for p in postings:
        if p.get("amount") is None:
            assert elided is None, "only one posting may elide its amount"
            elided = p
            continue
        weight, weight_cur = p["amount"], p["currency"]
        if p.get("cost") is not None:
            weight = d(p["amount"] * p["cost"])
            weight_cur = "USD"
        elif p.get("price") is not None:
            weight = d(p["amount"] * p["price"])
            weight_cur = "USD"
        residual[weight_cur] += weight
        BOOK.post(p["account"], p["amount"], p["currency"])

    if elided is not None:
        assert len(residual) == 1, f"elision needs one currency, got {dict(residual)}"
        cur, total = next(iter(residual.items()))
        # Left off the page entirely, the way beancount is written: the
        # residual is exact there and would only lose precision if this
        # rounded it to something printable.
        elided["amount"], elided["currency"] = -total, cur
        BOOK.post(elided["account"], -total, cur)

    for p in postings:
        if p is elided:
            lines.append(f"  {p['account']}")
            continue
        amount = p["amount"]
        places = 4 if p["currency"] not in ("USD", "EUR") else 2
        shown = f"{units(amount, places) if places == 4 else d(amount)}"
        line = f"  {p['account']:<38}{shown:>14} {p['currency']}"
        if p.get("cost") is not None:
            line += f" {{{d(p['cost'])} USD}}"
        if p.get("price") is not None:
            line += f" @ {d(p['price'])} USD"
        lines.append(line)
        for key, value in p.get("meta", ()):
            lines.append(f'    {key}: "{value}"')
    BOOK.add(when, "\n".join(lines))


def simple(when, payee, narration, expense, amount, funding, **kw):
    txn(when, "*", payee, narration,
        [{"account": expense, "amount": d(amount), "currency": "USD"},
         {"account": funding}], **kw)


# --------------------------------------------------------------------------
# Declarations
# --------------------------------------------------------------------------

COMMODITIES = [
    ("USD", "US Dollar", "cash"),
    ("EUR", "Euro", "cash"),
    ("VSTX", "Total US Market Index", "us-equity"),
    ("VINX", "Developed International Index", "intl-equity"),
    ("VEMX", "Emerging Markets Index", "intl-equity"),
    ("VBDX", "Aggregate Bond Index", "bonds"),
    ("VREX", "REIT Index", "real-estate"),
    ("GLDT", "Gold Bullion Trust", "commodities"),
    ("BTC", "Bitcoin", "crypto"),
    ("ETH", "Ether", "crypto"),
    ("MESH", "Mesh Protocol Token", "crypto"),
]

# account, currencies, name, extra metadata
ACCOUNTS = [
    ("Assets:Bank:Checking", "USD", "Checking", []),
    ("Assets:Bank:Savings", "USD", "Savings", []),
    ("Assets:Bank:Business", "USD", "Business Account", []),
    ("Assets:Cash:Wallet", "USD", "Cash", []),
    ("Assets:Prepaid:Transit", "USD", "Transit Card", []),
    ("Assets:Prepaid:Games", "USD", "Game Wallet", []),
    ("Assets:Broker:Cash", "USD", "Brokerage Cash", [("ynab", "tracking")]),
    ("Assets:Broker:VSTX", "VSTX", "US Market", []),
    ("Assets:Broker:VINX", "VINX", "International", []),
    ("Assets:Broker:VEMX", "VEMX", "Emerging Markets", []),
    ("Assets:Broker:VBDX", "VBDX", "Bonds", []),
    ("Assets:Broker:VREX", "VREX", "Real Estate", []),
    ("Assets:Retirement:VSTX", "VSTX", "Retirement — US Market", []),
    ("Assets:Retirement:VBDX", "VBDX", "Retirement — Bonds", []),
    ("Assets:Vault:GLDT", "GLDT", "Gold", []),
    ("Assets:Crypto:BTC", "BTC", "Bitcoin", []),
    ("Assets:Crypto:ETH", "ETH", "Ether", []),
    ("Assets:Crypto:MESH", "MESH", "Mesh Token", []),
    ("Assets:Old:Sunset", "USD", "Closed Account", [("ynab", "hidden")]),
    ("Assets:Vehicle:Car", "USD", "The Car", [("ynab", "tracking")]),
    ("Liabilities:Card:Everyday", "USD", "Everyday Card", [("limit", 8000)]),
    ("Liabilities:Card:Travel", "USD, EUR", "Travel Card", [("limit", 6000)]),
    ("Liabilities:Card:Store", "USD", "Store Card", [("limit", 5000)]),
    ("Liabilities:Loan:Car", "USD", "Car Loan",
     [("collateral", "Assets:Vehicle:Car")]),
    ("Liabilities:Loan:Student", "USD", "Student Loan", []),
    ("Liabilities:Loan:Furniture", "USD", "Furniture Financing",
     [("rate", 0), ("due", 15)]),
    ("Income:Halloran", "USD", "Halloran Systems", [("income", "active")]),
    ("Income:Cartwright", "USD", "Cartwright Group", [("income", "active")]),
    ("Income:Projects", "USD", "Project Work", [("income", "active")]),
    ("Income:Royalties", "USD", "Course Royalties", [("income", "passive")]),
    ("Income:Investments:Dividends", "USD", "Dividends", [("income", "passive")]),
    ("Income:Investments:Interest", "USD", "Interest", [("income", "passive")]),
    ("Income:Investments:Gains", "USD", "Realized Gains", [("income", "passive")]),
    ("Income:Investments:Staking", "ETH", "Staking Rewards", [("income", "passive")]),
    ("Income:Investments:Airdrops", "MESH", "Airdrops", [("income", "passive")]),
    ("Equity:Opening-Balances", "USD", None, []),
    ("Equity:Revaluation", "USD", None, []),
]

EXPENSES = [
    ("Expenses:Home:Rent", "Rent"),
    ("Expenses:Home:Utilities:Power", "Power"),
    ("Expenses:Home:Utilities:Water", "Water"),
    ("Expenses:Home:Utilities:Internet", "Internet"),
    ("Expenses:Home:Utilities:Phone", "Phone"),
    ("Expenses:Home:Maintenance", "Maintenance"),
    ("Expenses:Home:Furnishing", "Furnishing"),
    ("Expenses:Food:Groceries", "Groceries"),
    ("Expenses:Food:Dining", "Dining Out"),
    ("Expenses:Food:Coffee", "Coffee"),
    ("Expenses:Food:Delivery", "Delivery"),
    ("Expenses:Transport:Transit", "Transit"),
    ("Expenses:Transport:Rideshare", "Rideshare"),
    ("Expenses:Transport:Fuel", "Fuel"),
    ("Expenses:Transport:Parking", "Parking"),
    ("Expenses:Transport:Service", "Car Service"),
    ("Expenses:Health:Insurance", "Health Insurance"),
    ("Expenses:Health:Pharmacy", "Pharmacy"),
    ("Expenses:Health:Dental", "Dental"),
    ("Expenses:Health:Gym", "Gym"),
    ("Expenses:Subscriptions:Streaming", "Streaming"),
    ("Expenses:Subscriptions:Music", "Music"),
    ("Expenses:Subscriptions:News", "News"),
    ("Expenses:Subscriptions:Cloud", "Cloud Storage"),
    ("Expenses:Shopping:Clothes", "Clothes"),
    ("Expenses:Shopping:Electronics", "Electronics"),
    ("Expenses:Shopping:Household", "Household"),
    ("Expenses:Shopping:Books", "Books"),
    ("Expenses:Entertainment:Games", "Games"),
    ("Expenses:Entertainment:Events", "Events"),
    ("Expenses:Entertainment:Movies", "Movies"),
    ("Expenses:Travel:Flights", "Flights"),
    ("Expenses:Travel:Lodging", "Lodging"),
    ("Expenses:Travel:Local", "Travel — Local"),
    ("Expenses:Business:Software", "Business Software"),
    ("Expenses:Business:Accounting", "Accounting"),
    ("Expenses:Business:Hardware", "Business Hardware"),
    ("Expenses:Business:Coworking", "Coworking"),
    ("Expenses:Taxes:Income", "Income Tax"),
    ("Expenses:Taxes:SelfEmployment", "Self-Employment Tax"),
    ("Expenses:Financial:Fees", "Bank Fees"),
    ("Expenses:Financial:Interest", "Loan Interest"),
    ("Expenses:Gifts", "Gifts"),
    ("Expenses:Charity", "Charity"),
    ("Expenses:Misc", "Miscellaneous"),
    ("Expenses:Uncategorized", "Uncategorized"),
]


# --------------------------------------------------------------------------
# Prices — one deterministic walk per commodity, quoted at month end
# --------------------------------------------------------------------------

def months(start, end):
    y, m = start.year, start.month
    while (y, m) <= (end.year, end.month):
        yield y, m
        y, m = (y + 1, 1) if m == 12 else (y, m + 1)


def month_end(y, m):
    return date(y + (m == 12), 1 if m == 12 else m + 1, 1) - timedelta(days=1)


MONTHS = list(months(START, TODAY))

WALKS = {
    "VSTX": (108.40, 0.0072, 0.030),
    "VINX": (62.15, 0.0041, 0.026),
    "VEMX": (44.80, 0.0035, 0.038),
    "VBDX": (71.60, 0.0009, 0.009),
    "VREX": (84.20, 0.0022, 0.031),
    "GLDT": (191.30, 0.0064, 0.024),
    "BTC": (58_400.00, 0.0140, 0.115),
    "ETH": (2_940.00, 0.0095, 0.128),
    "EUR": (1.0850, 0.0006, 0.014),
}

PRICES = {}
for symbol, (base, drift, vol) in WALKS.items():
    walk = random.Random(SEED + sum(symbol.encode()))
    value, series = base, {}
    for y, m in MONTHS:
        value *= 1 + drift + walk.gauss(0, vol)
        places = 4 if symbol == "EUR" else 2
        series[(y, m)] = Decimal(str(round(value, places)))
    PRICES[symbol] = series


def price(symbol, when) -> Decimal:
    return PRICES[symbol][(when.year, when.month)]


# Gold stops being quoted after this: still held, no fresh price. The
# "what's missing" card exists to notice exactly that.
GOLD_LAST_QUOTE = (2025, 6)


# --------------------------------------------------------------------------
# Merchants — every one invented
# --------------------------------------------------------------------------

SAID = {
    "coffee": ["flat white", "filter coffee", "cold brew", "espresso and a bun"],
    "dining": ["dinner", "lunch", "dinner out", "late lunch", "supper"],
    "delivery": ["delivery", "friday takeaway", "dinner in"],
    "ride": ["ride home", "ride across town", "airport run", "ride to the desk"],
    "fuel": ["fill-up"],
    "pharmacy": ["prescription", "sundries"],
    "household": ["cleaning supplies", "kitchen bits", "hardware"],
    "clothes": ["jacket", "shoes", "shirts", "winter coat", "trousers"],
    "books": ["books"],
    "game": ["a game", "a game on sale", "expansion"],
    "movies": ["cinema"],
    "bill": ["monthly bill"],
    "plan": ["monthly plan"],
}


def said(kind):
    return rng.choice(SAID[kind])


GROCERS = ["Marketvale", "Green Basket", "Corner Grocer", "Harvest Lane"]
COFFEE = ["Ember Coffee", "Third Rail Coffee", "Meridian Roasters"]
DINING = ["Saltbox", "Nori House", "Trattoria Verde", "The Blue Kettle",
          "Pike & Ash", "Copper Spoon"]
DELIVERY = ["Swiftplate"]
RIDES = ["Loop Rides"]
FUEL = ["Halcyon Fuel"]
PHARMACY = ["Wellspring Pharmacy"]
HOUSEHOLD = ["Homestead Supply", "Tidewater Hardware"]
CLOTHES = ["Northwind Apparel", "Foundry Goods"]
ELECTRONICS = ["Voltcart"]
BOOKS = ["Marginalia Books"]
GAMES = ["Pixelforge Store"]
EVENTS = ["Odeon Row", "The Lantern Hall"]
MOVIES = ["Grand Marquee"]

# Monthly, on a stable day: this is the fixed nut the recurring card finds.
def rent_amount(when):
    return d(2450.00) if when < date(2025, 10, 1) else d(2595.00)


def streaming_amount(when):
    return d(15.99) if when < date(2025, 6, 1) else d(17.99)


def cloud_amount(when):
    return d(9.99) if when < date(2026, 2, 1) else d(11.99)


def insurance_amount(when):
    return d(385.00) if when < date(2026, 1, 1) else d(412.00)


def retainer_amount(when):
    return d(16_500.00) if when < date(2025, 7, 1) else d(18_200.00)


# Power is seasonal on purpose: the seasonality card has to have something
# to find, and heating and cooling are what a real ledger shows.
POWER_SHAPE = {1: 1.55, 2: 1.48, 3: 1.15, 4: 0.92, 5: 0.86, 6: 1.05,
               7: 1.42, 8: 1.51, 9: 1.12, 10: 0.90, 11: 1.06, 12: 1.34}


# --------------------------------------------------------------------------
# The month
# --------------------------------------------------------------------------

CARD = "Liabilities:Card:Everyday"
TRAVEL_CARD = "Liabilities:Card:Travel"
STORE_CARD = "Liabilities:Card:Store"
CAR_LOAN = "Liabilities:Loan:Car"
STUDENT_LOAN = "Liabilities:Loan:Student"
FURNITURE = "Liabilities:Loan:Furniture"
CAR = "Assets:Vehicle:Car"
CHECKING = "Assets:Bank:Checking"
SAVINGS = "Assets:Bank:Savings"
BUSINESS = "Assets:Bank:Business"
WALLET = "Assets:Cash:Wallet"
BROKER = "Assets:Broker:Cash"

DOCUMENTS = []   # (date, account, relative path, kind) — files written later


def day(y, m, dd):
    """The dd-th of the month, or its last day when it has fewer. Whether
    that day is inside the ledger is `in_month`'s question."""
    last = month_end(y, m).day
    return date(y, m, min(dd, last))


def in_month(when, y, m):
    return when.year == y and when.month == m and when <= LAST


def spread(y, m, count, lo=2, hi=27):
    """`count` distinct days in the month, in order."""
    top = min(hi, month_end(y, m).day)
    if LAST.year == y and LAST.month == m:
        top = min(top, LAST.day)
    span = list(range(lo, top + 1))
    if len(span) <= count:
        return span
    return sorted(rng.sample(span, count))


def document(when, account, folder, name, body):
    rel = f"documents/{when.year}/{when.month:02d}/{name}"
    DOCUMENTS.append((rel, body))
    BOOK.add(when, f'{when} document {account} "{rel}"', sort=2)


def housing(y, m):
    when = day(y, m, 1)
    if in_month(when, y, m):
        simple(when, "Ridgeline Properties", "monthly rent",
               "Expenses:Home:Rent", rent_amount(when), CHECKING)
    for dd, payee, account, lo, hi, shape in [
        (9, "Gridworks Power", "Expenses:Home:Utilities:Power", 88, 132, True),
        (11, "Clearwater Utility", "Expenses:Home:Utilities:Water", 41, 59, False),
        (6, "Fiberline", "Expenses:Home:Utilities:Internet", 79, 79, False),
        (14, "Cellmark", "Expenses:Home:Utilities:Phone", 45, 45, False),
    ]:
        when = day(y, m, dd)
        if not in_month(when, y, m):
            continue
        amount = money(rng, lo, hi)
        if shape:
            amount = d(amount * Decimal(str(POWER_SHAPE[m])))
        simple(when, payee, said("bill"), account, amount, CHECKING)


def subscriptions(y, m):
    plan = [
        (3, "Streamflix", "Expenses:Subscriptions:Streaming", streaming_amount(date(y, m, 1))),
        (5, "Tunebox", "Expenses:Subscriptions:Music", d(10.99)),
        (8, "PixelPress", "Expenses:Subscriptions:News", d(12.00)),
        (12, "Nimbus Drive", "Expenses:Subscriptions:Cloud", cloud_amount(date(y, m, 1))),
        (17, "Photobank", "Expenses:Subscriptions:Cloud", d(8.99)),
        (2, "Forge IDE", "Expenses:Business:Software", d(25.00)),
        (21, "IronWorks Gym", "Expenses:Health:Gym", d(65.00)),
    ]
    for dd, payee, account, amount in plan:
        when = day(y, m, dd)
        if in_month(when, y, m):
            simple(when, payee, said("plan"), account, amount, CARD)

    when = day(y, m, 4)
    if in_month(when, y, m):
        simple(when, "Meridian Health", "monthly premium",
               "Expenses:Health:Insurance", insurance_amount(when), CHECKING)
    when = day(y, m, 2)
    if in_month(when, y, m):
        simple(when, "The Annex", "coworking desk",
               "Expenses:Business:Coworking", d(300.00), BUSINESS)
    when = day(y, m, 15)
    if in_month(when, y, m):
        simple(when, "Sentry Books", "bookkeeping retainer",
               "Expenses:Business:Accounting", d(180.00), BUSINESS)


def car_loan(y, m):
    when = day(y, m, 18)
    if not in_month(when, y, m):
        return
    outstanding = -BOOK.balance(CAR_LOAN)
    interest = d(outstanding * Decimal("0.0044"))
    principal = d(Decimal("465.00") - interest)
    txn(when, "*", "Northgate Auto Finance", "car payment", [
        {"account": CAR_LOAN, "amount": principal, "currency": "USD"},
        {"account": "Expenses:Financial:Interest", "amount": interest, "currency": "USD"},
        {"account": CHECKING},
    ])


def car_value(y, m):
    """What the car would fetch, a little less every quarter, marked
    down against equity: the loan is secured on it, and a loan measured
    against nothing is just a number."""
    if m not in (3, 6, 9, 12):
        return
    when = month_end(y, m)
    if when > LAST:
        return
    age = (y - START.year) * 12 + (m - START.month) + 1
    worth = d(Decimal("21900") * Decimal("0.986") ** age)
    drop = d(BOOK.balance(CAR) - worth)
    if drop <= 0:
        return
    txn(when, "*", None, "the car, marked to what it would fetch", [
        {"account": CAR, "amount": -drop, "currency": "USD"},
        {"account": "Equity:Revaluation"},
    ])


def student_loan(y, m):
    """Paid on the 5th until nothing is left, then closed the month
    after: a debt the page can only show as beaten."""
    when = day(y, m, 5)
    if not in_month(when, y, m):
        return
    outstanding = -BOOK.balance(STUDENT_LOAN)
    if outstanding <= 0:
        return
    interest = d(outstanding * Decimal("0.00375"))
    principal = min(d(Decimal("290.00") - interest), outstanding)
    txn(when, "*", "Meridian Student Lending", "student loan payment", [
        {"account": STUDENT_LOAN, "amount": principal, "currency": "USD"},
        {"account": "Expenses:Financial:Interest", "amount": interest, "currency": "USD"},
        {"account": CHECKING},
    ])
    if principal >= outstanding:
        closed = date(y + (m == 12), 1 if m == 12 else m + 1, 1)
        BOOK.add(closed, f"{closed} close {STUDENT_LOAN}", sort=3)


def furniture(y, m):
    """A sofa on two years of interest-free credit. No interest leg will
    ever say what the rate is, and one payment is not enough to say
    when it is due, so the open directive says both."""
    if (y, m) == (2026, 7):
        txn(date(2026, 7, 20), "*", "Alder & Oak", "sofa, 24 months interest-free", [
            {"account": "Expenses:Home:Furnishing", "amount": d(1_800.00),
             "currency": "USD"},
            {"account": FURNITURE},
        ])
    when = day(y, m, 15)
    owed = -BOOK.balance(FURNITURE)
    if owed > 0 and in_month(when, y, m):
        txn(when, "*", "Alder & Oak", "instalment", [
            {"account": FURNITURE, "amount": min(d(75.00), owed), "currency": "USD"},
            {"account": CHECKING},
        ])


# The store card draws from its own generator so that adding it did not
# re-roll every month of everything else.
STORE_FROM = (2025, 10)
store_rng = random.Random(SEED + 0x5702E)


def store_card(y, m):
    """A store card taken out in the autumn of 2025 and never cleared
    since: interest on the 1st, the minimum on the 10th, and more put
    on it every month than comes off."""
    if (y, m) < STORE_FROM:
        return
    when = day(y, m, 1)
    owed = -BOOK.balance(STORE_CARD)
    if owed > 0 and in_month(when, y, m):
        simple(when, "Hearthside Home", "interest charge",
               "Expenses:Financial:Interest", d(owed * Decimal("0.0199")),
               STORE_CARD)
    when = day(y, m, 10)
    owed = -BOOK.balance(STORE_CARD)
    if owed > 0 and in_month(when, y, m):
        minimum = min(max(d(35.00), d(owed * Decimal("0.03"))), owed)
        txn(when, "*", "Hearthside Home", "minimum payment", [
            {"account": STORE_CARD, "amount": minimum, "currency": "USD"},
            {"account": CHECKING},
        ], tags=["autopay"])
    top = month_end(y, m).day
    if LAST.year == y and LAST.month == m:
        top = min(top, LAST.day)
    days = list(range(3, min(top, 27) + 1))
    count = min(len(days), store_rng.randint(1, 2))
    for dd in sorted(store_rng.sample(days, count)):
        simple(day(y, m, dd), "Hearthside Home", store_rng.choice(SAID["household"]),
               store_rng.choice(["Expenses:Shopping:Household",
                                 "Expenses:Home:Furnishing"]),
               money(store_rng, 60, 260), STORE_CARD)


def everyday_spend(y, m):
    for dd in spread(y, m, 4, 2, 27):
        when = day(y, m, dd)
        simple(when, rng.choice(GROCERS), "weekly shop",
               "Expenses:Food:Groceries", money(rng, 74, 208), CARD)
    for dd in spread(y, m, rng.randint(8, 13), 2, 27):
        simple(day(y, m, dd), rng.choice(COFFEE), said("coffee"),
               "Expenses:Food:Coffee", money(rng, 4.20, 9.40), CARD)
    for dd in spread(y, m, rng.randint(4, 7), 2, 27):
        simple(day(y, m, dd), rng.choice(DINING), said("dining"),
               "Expenses:Food:Dining", money(rng, 31, 148), CARD)
    for dd in spread(y, m, rng.randint(2, 5), 2, 27):
        simple(day(y, m, dd), rng.choice(DELIVERY), said("delivery"),
               "Expenses:Food:Delivery", money(rng, 21, 62), CARD)
    for dd in spread(y, m, rng.randint(4, 9), 2, 27):
        simple(day(y, m, dd), rng.choice(RIDES), said("ride"),
               "Expenses:Transport:Rideshare", money(rng, 8.50, 39), CARD)
    for dd in spread(y, m, 2, 3, 26):
        simple(day(y, m, dd), rng.choice(FUEL), said("fuel"),
               "Expenses:Transport:Fuel", money(rng, 44, 71), CARD)
    when = day(y, m, 7)
    if in_month(when, y, m):
        txn(when, "*", "CityTransit", "card top-up", [
            {"account": "Assets:Prepaid:Transit", "amount": d(60.00), "currency": "USD"},
            {"account": CHECKING},
        ])
    for dd in spread(y, m, rng.randint(6, 12), 2, 27):
        when = day(y, m, dd)
        simple(when, "CityTransit", "fare", "Expenses:Transport:Transit",
               money(rng, 2.40, 6.20), "Assets:Prepaid:Transit")
    for dd in spread(y, m, rng.randint(1, 2), 4, 26):
        simple(day(y, m, dd), rng.choice(PHARMACY), said("pharmacy"),
               "Expenses:Health:Pharmacy", money(rng, 11, 92), CARD)
    for dd in spread(y, m, rng.randint(2, 3), 3, 26):
        simple(day(y, m, dd), rng.choice(HOUSEHOLD), said("household"),
               "Expenses:Shopping:Household", money(rng, 22, 134), CARD)

    # Cash: withdrawn, then spent without a merchant name to hang it on.
    when = day(y, m, 13)
    if in_month(when, y, m):
        txn(when, "*", "Harborline Bank", "cash withdrawal", [
            {"account": WALLET, "amount": d(200.00), "currency": "USD"},
            {"account": CHECKING},
        ])
    for dd in spread(y, m, rng.randint(2, 4), 14, 27):
        simple(day(y, m, dd), None, "cash", "Expenses:Misc",
               money(rng, 8, 44), WALLET)


def discretionary(y, m):
    if rng.random() < 0.55:
        for dd in spread(y, m, 1, 5, 26):
            simple(day(y, m, dd), rng.choice(CLOTHES), said("clothes"),
                   "Expenses:Shopping:Clothes", money(rng, 48, 320), CARD)
    if rng.random() < 0.35:
        for dd in spread(y, m, 1, 5, 26):
            simple(day(y, m, dd), rng.choice(BOOKS), said("books"),
                   "Expenses:Shopping:Books", money(rng, 16, 74), CARD)
    if rng.random() < 0.45:
        for dd in spread(y, m, 1, 5, 26):
            when = day(y, m, dd)
            txn(when, "*", "Pixelforge Store", "wallet top-up", [
                {"account": "Assets:Prepaid:Games", "amount": d(50.00), "currency": "USD"},
                {"account": CARD},
            ])
    if rng.random() < 0.6:
        for dd in spread(y, m, rng.randint(1, 3), 5, 26):
            simple(day(y, m, dd), rng.choice(GAMES), said("game"),
                   "Expenses:Entertainment:Games", money(rng, 8, 62),
                   "Assets:Prepaid:Games")
    if rng.random() < 0.5:
        for dd in spread(y, m, 1, 5, 26):
            simple(day(y, m, dd), rng.choice(MOVIES), said("movies"),
                   "Expenses:Entertainment:Movies", money(rng, 22, 58), CARD)
    if rng.random() < 0.3:
        for dd in spread(y, m, 1, 5, 26):
            simple(day(y, m, dd), rng.choice(EVENTS), "tickets",
                   "Expenses:Entertainment:Events", money(rng, 70, 260), CARD)
    if rng.random() < 0.25:
        for dd in spread(y, m, 1, 5, 26):
            simple(day(y, m, dd), "Voltcart", "gadgets",
                   "Expenses:Shopping:Electronics", money(rng, 42, 340), CARD)
    if m in (11, 12):
        for dd in spread(y, m, rng.randint(2, 4), 4, 22):
            simple(day(y, m, dd), rng.choice(["Northwind Apparel", "Voltcart",
                                              "Marginalia Books"]),
                   "gift", "Expenses:Gifts", money(rng, 40, 210), CARD)
    when = day(y, m, 20)
    if in_month(when, y, m):
        simple(when, "Open Harbor Fund", "monthly giving",
               "Expenses:Charity", d(150.00), CHECKING)


def buy(when, account, symbol, dollars, payee="Meridian Brokerage",
        narration="scheduled buy", funding=BROKER, links=()):
    unit_price = price(symbol, when)
    qty = units(Decimal(str(dollars)) / unit_price)
    txn(when, "*", payee, narration, [
        {"account": account, "amount": qty, "currency": symbol,
         "cost": unit_price},
        {"account": funding},
    ], links=links)
    return qty


def income(y, m):
    when = day(y, m, 1)
    if in_month(when, y, m):
        txn(when, "*", "Halloran Systems", "monthly retainer", [
            {"account": BUSINESS, "amount": retainer_amount(when), "currency": "USD"},
            {"account": "Income:Halloran"},
        ], links=[f"inv-{y}-{m:02d}"])
    when = day(y, m, 3)
    if in_month(when, y, m):
        txn(when, "*", "Cartwright Group", "advisory retainer", [
            {"account": BUSINESS, "amount": d(3_500.00), "currency": "USD"},
            {"account": "Income:Cartwright"},
        ])
    when = day(y, m, 11)
    if in_month(when, y, m):
        txn(when, "*", "Lightfoot Press", "course royalties", [
            {"account": BUSINESS, "amount": money(rng, 280, 940), "currency": "USD"},
            {"account": "Income:Royalties"},
        ])
    when = day(y, m, 5)
    if in_month(when, y, m):
        txn(when, "*", None, "owner draw", [
            {"account": CHECKING, "amount": d(18_300.00), "currency": "USD"},
            {"account": BUSINESS},
        ])
    when = day(y, m, 28)
    if in_month(when, y, m):
        earned = d(max(BOOK.balance(SAVINGS), Decimal(0)) * Decimal("0.0036"))
        if earned > 0:
            txn(when, "*", "Harborline Bank", "savings interest", [
                {"account": SAVINGS, "amount": earned, "currency": "USD"},
                {"account": "Income:Investments:Interest"},
            ])
    # Staking arrives as ether, never as a purchase: nothing about it
    # records a cost, which is what the investments card has to say.
    when = day(y, m, 22)
    if in_month(when, y, m):
        txn(when, "*", "Tideworks Staking", "staking reward", [
            {"account": "Assets:Crypto:ETH", "amount": units(rng.uniform(0.031, 0.058)),
             "currency": "ETH"},
            {"account": "Income:Investments:Staking"},
        ])
    if rng.random() < 0.45:
        for dd in spread(y, m, 1, 6, 24):
            when = day(y, m, dd)
            client = rng.choice(["Pinehurst Labs", "Cardinal Freight",
                                 "Vellum Analytics", "Northcape Media"])
            txn(when, "*", client, "project invoice", [
                {"account": BUSINESS, "amount": money(rng, 4_200, 17_800),
                 "currency": "USD"},
                {"account": "Income:Projects"},
            ], links=[f"po-{y}{m:02d}{dd:02d}"])


def quarterly(y, m):
    if m in (1, 4, 7, 10):
        when = day(y, m, 16)
        if in_month(when, y, m):
            txn(when, "*", "Revenue Service", "estimated tax", [
                {"account": "Expenses:Taxes:Income", "amount": d(16_500.00),
                 "currency": "USD"},
                {"account": "Expenses:Taxes:SelfEmployment", "amount": d(4_500.00),
                 "currency": "USD"},
                {"account": BUSINESS},
            ])
    if m in (3, 6, 9, 12):
        when = day(y, m, 24)
        if in_month(when, y, m):
            txn(when, "*", "Meridian Brokerage", "quarterly dividends", [
                {"account": BROKER, "amount": money(rng, 610, 1_480),
                 "currency": "USD"},
                {"account": "Income:Investments:Dividends"},
            ])
        when = day(y, m, 12)
        if in_month(when, y, m):
            simple(when, "Harborline Bank", "account fees",
                   "Expenses:Financial:Fees", d(24.00), CHECKING)


def investing(y, m):
    when = day(y, m, 6)
    if in_month(when, y, m):
        txn(when, "*", None, "transfer to brokerage", [
            {"account": BROKER, "amount": d(8_000.00), "currency": "USD"},
            {"account": CHECKING},
        ])
    when = day(y, m, 8)
    if in_month(when, y, m):
        buy(when, "Assets:Broker:VSTX", "VSTX", 4_700)
        buy(when, "Assets:Broker:VINX", "VINX", 1_850)
        buy(when, "Assets:Broker:VBDX", "VBDX", 1_150)
    if m in (2, 5, 8, 11):
        when = day(y, m, 9)
        if in_month(when, y, m):
            buy(when, "Assets:Broker:VEMX", "VEMX", 620)
    when = day(y, m, 10)
    if in_month(when, y, m):
        txn(when, "*", None, "retirement contribution", [
            {"account": BROKER, "amount": d(1_800.00), "currency": "USD"},
            {"account": CHECKING},
        ])
        buy(when, "Assets:Retirement:VSTX", "VSTX", 1_300, narration="retirement buy")
        buy(when, "Assets:Retirement:VBDX", "VBDX", 500, narration="retirement buy")
    when = day(y, m, 19)
    if in_month(when, y, m):
        txn(when, "*", None, "transfer to savings", [
            {"account": SAVINGS, "amount": d(1_200.00), "currency": "USD"},
            {"account": CHECKING},
        ])
    if rng.random() < 0.4:
        for dd in spread(y, m, 1, 11, 25):
            when = day(y, m, dd)
            symbol = rng.choice(["BTC", "ETH"])
            account = f"Assets:Crypto:{symbol}"
            txn(when, "*", "Anchorpoint Exchange", "recurring buy", [
                {"account": account,
                 "amount": units(Decimal(str(rng.uniform(240, 620))) / price(symbol, when), 6),
                 "currency": symbol, "cost": price(symbol, when)},
                {"account": CHECKING},
            ])


def pay_cards(y, m):
    for card, source, dd in [(CARD, CHECKING, 26), (TRAVEL_CARD, CHECKING, 27)]:
        when = day(y, m, dd)
        if not in_month(when, y, m):
            continue
        owed = -BOOK.balance(card)
        if owed <= 0:
            continue
        txn(when, "*", "Harborline Bank", "statement payment", [
            {"account": card, "amount": d(owed), "currency": "USD"},
            {"account": source},
        ], tags=["autopay"])


# --------------------------------------------------------------------------
# The things that only happen once
# --------------------------------------------------------------------------

def opening():
    txn(START, "*", None, "opening balances", [
        {"account": CHECKING, "amount": d(34_000.00), "currency": "USD"},
        {"account": SAVINGS, "amount": d(41_800.00), "currency": "USD"},
        {"account": BUSINESS, "amount": d(68_000.00), "currency": "USD"},
        {"account": WALLET, "amount": d(240.00), "currency": "USD"},
        {"account": "Assets:Prepaid:Transit", "amount": d(38.00), "currency": "USD"},
        {"account": "Assets:Prepaid:Games", "amount": d(12.50), "currency": "USD"},
        {"account": BROKER, "amount": d(3_100.00), "currency": "USD"},
        {"account": "Assets:Old:Sunset", "amount": d(310.00), "currency": "USD"},
        {"account": CAR, "amount": d(21_900.00), "currency": "USD"},
        {"account": CAR_LOAN, "amount": d(-18_400.00), "currency": "USD"},
        {"account": STUDENT_LOAN, "amount": d(-6_240.00), "currency": "USD"},
        {"account": "Equity:Opening-Balances"},
    ])
    txn(START, "*", None, "opening positions", [
        {"account": "Assets:Broker:VSTX", "amount": units(612.4180),
         "currency": "VSTX", "cost": d(84.10)},
        {"account": "Assets:Broker:VINX", "amount": units(318.9040),
         "currency": "VINX", "cost": d(55.30)},
        {"account": "Assets:Broker:VBDX", "amount": units(204.2200),
         "currency": "VBDX", "cost": d(69.80)},
        {"account": "Assets:Retirement:VSTX", "amount": units(486.1120),
         "currency": "VSTX", "cost": d(79.40)},
        {"account": "Assets:Retirement:VBDX", "amount": units(160.0000),
         "currency": "VBDX", "cost": d(70.20)},
        {"account": "Assets:Crypto:BTC", "amount": units(0.412000, 6),
         "currency": "BTC", "cost": d(41_200.00)},
        {"account": "Assets:Crypto:ETH", "amount": units(3.180000, 6),
         "currency": "ETH", "cost": d(2_180.00)},
        {"account": "Equity:Opening-Balances"},
    ])


def one_offs(y, m):
    # Gold, bought once and then quietly left unquoted from mid-2025.
    if (y, m) == (2024, 10):
        buy(date(2024, 10, 22), "Assets:Vault:GLDT", "GLDT", 9_400,
            payee="Bullion Vault", narration="allocated gold", funding=SAVINGS)
    # An airdrop nobody asked for and no market ever priced.
    if (y, m) == (2024, 11):
        txn(date(2024, 11, 14), "*", "Anchorpoint Exchange", "protocol airdrop", [
            {"account": "Assets:Crypto:MESH", "amount": units(1_400.0000),
             "currency": "MESH"},
            {"account": "Income:Investments:Airdrops"},
        ])
    if (y, m) == (2024, 12):
        txn(date(2024, 12, 3), "*", "Harborline Bank", "close old account", [
            {"account": CHECKING, "amount": d(310.00), "currency": "USD"},
            {"account": "Assets:Old:Sunset"},
        ])
        BOOK.add(date(2024, 12, 4), "2024-12-04 close Assets:Old:Sunset", sort=3)

    if (y, m) == (2025, 2):
        when = date(2025, 2, 11)
        txn(when, "*", None, "fund the real estate sleeve", [
            {"account": BROKER, "amount": d(10_200.00), "currency": "USD"},
            {"account": SAVINGS},
        ])
        STATE["vrex_cost"] = price("VREX", when)
        buy(when, "Assets:Broker:VREX", "VREX", 10_104,
            narration="real estate sleeve")

    # The renovation: one tag, five months, four categories. This is what
    # the projects card is for.
    if (y, m) == (2025, 4):
        simple(date(2025, 4, 8), "Tidewater Hardware", "demolition and haul-away",
               "Expenses:Home:Maintenance", d(2_340.00), CARD, tags=["kitchen-reno"])
        simple(date(2025, 4, 24), "Fairview Cabinetry", "cabinet deposit",
               "Expenses:Home:Furnishing", d(4_800.00), CHECKING, tags=["kitchen-reno"])
    if (y, m) == (2025, 5):
        simple(date(2025, 5, 16), "Meridian Plumbing", "rough-in",
               "Expenses:Home:Maintenance", d(1_950.00), CHECKING, tags=["kitchen-reno"])
        simple(date(2025, 5, 29), "Voltcart", "range and hood",
               "Expenses:Shopping:Electronics", d(3_180.00), CARD, tags=["kitchen-reno"])
    if (y, m) == (2025, 6):
        simple(date(2025, 6, 12), "Fairview Cabinetry", "installation",
               "Expenses:Home:Furnishing", d(5_600.00), CHECKING, tags=["kitchen-reno"])
    if (y, m) == (2025, 7):
        simple(date(2025, 7, 9), "Northline Electric", "circuits and lighting",
               "Expenses:Home:Maintenance", d(1_420.00), CHECKING, tags=["kitchen-reno"])
    if (y, m) == (2025, 8):
        simple(date(2025, 8, 5), "Homestead Supply", "tile and finish",
               "Expenses:Home:Maintenance", d(880.00), CARD, tags=["kitchen-reno"])

    if (y, m) == (2025, 9):
        lisbon()
    if (y, m) == (2026, 5):
        kyoto()
    if (y, m) == (2026, 8):
        porto()
        simple(date(2026, 8, 12), "Voltcart", "monitor",
               "Expenses:Shopping:Electronics", d(489.00), CARD)

    # One name on one transaction: a reference, not a project. The card
    # sets these aside and says how many.
    if (y, m) == (2026, 2):
        simple(date(2026, 2, 18), "Voltcart", "workstation",
               "Expenses:Business:Hardware", d(3_450.00), BUSINESS,
               tags=["new-laptop"])

    # Selling the real-estate sleeve whole, at a lot that recorded a cost.
    if (y, m) == (2026, 3):
        when = date(2026, 3, 17)
        cost = STATE["vrex_cost"]
        sale = price("VREX", when)
        qty = units(BOOK.balance("Assets:Broker:VREX", "VREX"))
        proceeds = d(qty * sale)
        txn(when, "*", "Meridian Brokerage", "sell the real estate sleeve", [
            {"account": "Assets:Broker:VREX", "amount": -qty, "currency": "VREX",
             "cost": cost, "price": sale},
            {"account": BROKER, "amount": proceeds, "currency": "USD"},
            {"account": "Income:Investments:Gains"},
        ])
        half = d(proceeds / 2)
        buy(date(2026, 3, 18), "Assets:Broker:VSTX", "VSTX", half,
            narration="reinvest the sleeve")
        buy(date(2026, 3, 18), "Assets:Broker:VINX", "VINX",
            d(proceeds - half - 20), narration="reinvest the sleeve")

    # The quarter the maintenance bill jumps, so the movers card has a mover.
    if (y, m) == (2026, 6):
        simple(date(2026, 6, 10), "Northline Electric", "panel replacement",
               "Expenses:Home:Maintenance", d(4_260.00), CHECKING)
        simple(date(2026, 6, 23), "Tidewater Hardware", "roof flashing",
               "Expenses:Home:Maintenance", d(1_180.00), CHECKING)

    # Dental turns up twice a year and nowhere else.
    if m in (3, 9):
        when = day(y, m, 21)
        if in_month(when, y, m):
            simple(when, "Rowan Dental", "cleaning",
                   "Expenses:Health:Dental", money(rng, 140, 420), CARD)
    if m == 4:
        when = day(y, m, 26)
        if in_month(when, y, m):
            simple(when, "Halcyon Service", "annual service",
                   "Expenses:Transport:Service", money(rng, 380, 720), CARD)

    # A few charges nobody has looked at yet.
    if rng.random() < 0.28:
        for dd in spread(y, m, 1, 6, 25):
            when = day(y, m, dd)
            txn(when, "!", None, "unrecognised card charge", [
                {"account": "Expenses:Uncategorized",
                 "amount": money(rng, 18, 240), "currency": "USD"},
                {"account": CARD},
            ])


def lisbon():
    """A week abroad, charged in euro. The card carries a foreign balance
    until it is settled, which is the only way the euro ever shows up in a
    balance rather than as a converted number."""
    trip = "trip-lisbon"
    simple(date(2025, 9, 6), "Meridian Air", "Lisbon, outbound",
           "Expenses:Travel:Flights", d(1_284.00), TRAVEL_CARD, links=[trip])
    eur = [
        (date(2025, 9, 13), "Casa da Praça", "lodging, four nights",
         "Expenses:Travel:Lodging", d(742.00)),
        (date(2025, 9, 14), "Taberna do Sol", "dinner",
         "Expenses:Food:Dining", d(68.50)),
        (date(2025, 9, 15), "Alfama Tram", "transit pass",
         "Expenses:Travel:Local", d(34.00)),
        (date(2025, 9, 16), "Café Ribeira", "coffee",
         "Expenses:Food:Coffee", d(9.80)),
        (date(2025, 9, 17), "Mercado Central", "market lunch",
         "Expenses:Food:Dining", d(41.20)),
        (date(2025, 9, 18), "Museu do Azulejo", "tickets",
         "Expenses:Entertainment:Events", d(28.00)),
    ]
    for when, payee, narration, account, amount in eur:
        txn(when, "*", payee, narration, [
            {"account": account, "amount": amount, "currency": "EUR"},
            {"account": TRAVEL_CARD, "amount": -amount, "currency": "EUR"},
        ], links=[trip])
    owed = -BOOK.balance(TRAVEL_CARD, "EUR")
    txn(date(2025, 10, 2), "*", "Harborline Bank", "travel card, euro balance", [
        {"account": TRAVEL_CARD, "amount": d(owed), "currency": "EUR",
         "price": price("EUR", date(2025, 10, 2))},
        {"account": CHECKING},
    ])


def porto():
    """The trip whose euros are still on the card: the flights were paid
    with the August statement, the week abroad was not."""
    trip = "trip-porto"
    simple(date(2026, 8, 3), "Meridian Air", "Porto, outbound",
           "Expenses:Travel:Flights", d(1_120.00), TRAVEL_CARD, links=[trip])
    for when, payee, narration, account, amount in [
        (date(2026, 8, 21), "Casa do Rio", "lodging, five nights",
         "Expenses:Travel:Lodging", d(615.00)),
        (date(2026, 8, 22), "Taberna Douro", "dinner",
         "Expenses:Food:Dining", d(74.30)),
        (date(2026, 8, 23), "Metro do Porto", "transit pass",
         "Expenses:Travel:Local", d(30.00)),
        (date(2026, 8, 24), "Café Majestic", "coffee",
         "Expenses:Food:Coffee", d(11.40)),
        (date(2026, 8, 25), "Mercado do Bolhão", "market lunch",
         "Expenses:Food:Dining", d(38.60)),
        (date(2026, 8, 26), "Livraria Lello", "books",
         "Expenses:Shopping:Books", d(52.00)),
        (date(2026, 8, 27), "Casa da Música", "tickets",
         "Expenses:Entertainment:Events", d(64.00)),
    ]:
        txn(when, "*", payee, narration, [
            {"account": account, "amount": amount, "currency": "EUR"},
            {"account": TRAVEL_CARD, "amount": -amount, "currency": "EUR"},
        ], links=[trip])


def refunds(y, m):
    """After the statement is paid, a return lands: the card owes its
    holder for a few days, which is the one way a liability goes
    negative. The purchase itself is in `one_offs`, so the statement
    that paid for it is the one before this runs."""
    if (y, m) == (2026, 8):
        simple(date(2026, 8, 30), "Voltcart", "monitor returned",
               "Expenses:Shopping:Electronics", d(-489.00), CARD)


def kyoto():
    trip = "trip-kyoto"
    for when, payee, narration, account, amount in [
        (date(2026, 5, 9), "Meridian Air", "Kyoto, outbound",
         "Expenses:Travel:Flights", d(1_940.00)),
        (date(2026, 5, 11), "Ryokan Kitano", "lodging, six nights",
         "Expenses:Travel:Lodging", d(2_310.00)),
        (date(2026, 5, 12), "Kamo Line", "rail pass",
         "Expenses:Travel:Local", d(288.00)),
        (date(2026, 5, 13), "Higashi Kitchen", "dinner",
         "Expenses:Food:Dining", d(164.00)),
        (date(2026, 5, 15), "Nishiki Market", "lunch",
         "Expenses:Food:Dining", d(72.00)),
        (date(2026, 5, 16), "Kurodani Temple", "entry",
         "Expenses:Entertainment:Events", d(46.00)),
    ]:
        simple(when, payee, narration, account, amount, TRAVEL_CARD, links=[trip])


# --------------------------------------------------------------------------
# Paperwork — real files, so the document rows in the panel actually open
# --------------------------------------------------------------------------

def receipt(title, rows, total):
    body = [
        "<!doctype html>",
        '<meta charset="utf-8">',
        f"<title>{title}</title>",
        "<style>body{font:14px/1.5 system-ui,sans-serif;max-width:34em;"
        "margin:3em auto;color:#222}h1{font-size:1.1em;letter-spacing:.04em;"
        "text-transform:uppercase}table{width:100%;border-collapse:collapse}"
        "td{padding:.4em 0;border-bottom:1px solid #eee}td:last-child{"
        "text-align:right;font-variant-numeric:tabular-nums}"
        "tr:last-child td{border:0;font-weight:600}</style>",
        f"<h1>{title}</h1>",
        "<table>",
    ]
    for label, amount in rows:
        body.append(f"<tr><td>{label}</td><td>{amount}</td></tr>")
    body.append(f"<tr><td>Total</td><td>{total}</td></tr>")
    body.append("</table>")
    body.append("<p>This is a sample document in a sample ledger. "
                "Nobody was billed.</p>")
    return "\n".join(body) + "\n"


def statement(rows):
    out = ["date,description,amount"]
    out += [f"{when},{what},{amount}" for when, what, amount in rows]
    return "\n".join(out) + "\n"


def paperwork():
    for y, m in [(2024, 10), (2025, 1), (2025, 4), (2025, 7), (2025, 10),
                 (2026, 1), (2026, 4), (2026, 7)]:
        when = day(y, m, 16)
        if not in_month(when, y, m):
            continue
        document(when, "Expenses:Taxes:Income", "tax",
                 f"{when}-estimated-tax.html",
                 receipt("Estimated tax payment",
                         [("Income tax", "16,500.00 USD"),
                          ("Self-employment tax", "4,500.00 USD")],
                         "21,000.00 USD"))
    for y, m in [(2024, 11), (2025, 3), (2025, 8), (2026, 2), (2026, 6)]:
        when = day(y, m, 1)
        if not in_month(when, y, m):
            continue
        amount = retainer_amount(when)
        document(when, "Assets:Bank:Business", "invoice",
                 f"{when}-invoice-halloran.html",
                 receipt(f"Invoice {y}-{m:02d} — Halloran Systems",
                         [("Retainer, one month", f"{amount} USD")],
                         f"{amount} USD"))
    for y, m in [(2025, 2), (2025, 9), (2026, 3), (2026, 7)]:
        when = day(y, m, 26)
        if not in_month(when, y, m):
            continue
        document(when, "Liabilities:Card:Everyday", "statement",
                 f"{when}-everyday-card.csv",
                 statement([
                     (f"{y}-{m:02d}-04", "MERIDIAN HEALTH", "-385.00"),
                     (f"{y}-{m:02d}-09", "MARKETVALE", "-142.18"),
                     (f"{y}-{m:02d}-14", "EMBER COFFEE", "-6.40"),
                     (f"{y}-{m:02d}-21", "IRONWORKS GYM", "-65.00"),
                 ]))
    for y, m in [(2025, 6), (2026, 5)]:
        when = day(y, m, 30 if m == 6 else 29)
        if not in_month(when, y, m):
            continue
        document(when, "Assets:Broker:Cash", "brokerage",
                 f"{when}-brokerage.csv",
                 statement([
                     (f"{y}-{m:02d}-08", "BUY VSTX", "-4700.00"),
                     (f"{y}-{m:02d}-08", "BUY VINX", "-1850.00"),
                     (f"{y}-{m:02d}-08", "BUY VBDX", "-1150.00"),
                 ]))
    document(date(2025, 4, 24), "Expenses:Home:Furnishing", "reno",
             "2025-04-24-cabinetry-quote.html",
             receipt("Fairview Cabinetry — quote",
                     [("Cabinet run, uppers", "2,640.00 USD"),
                      ("Cabinet run, base", "2,160.00 USD"),
                      ("Installation, scheduled June", "5,600.00 USD")],
                     "10,400.00 USD"))
    document(date(2025, 9, 6), "Expenses:Travel:Flights", "trip",
             "2025-09-06-lisbon-itinerary.html",
             receipt("Meridian Air — itinerary",
                     [("Outbound, seat 14A", "642.00 USD"),
                      ("Return, seat 9C", "642.00 USD")],
                     "1,284.00 USD"))
    document(date(2026, 5, 9), "Expenses:Travel:Flights", "trip",
             "2026-05-09-kyoto-itinerary.html",
             receipt("Meridian Air — itinerary",
                     [("Outbound, seat 22F", "970.00 USD"),
                      ("Return, seat 22F", "970.00 USD")],
                     "1,940.00 USD"))
    document(date(2026, 8, 3), "Expenses:Travel:Flights", "trip",
             "2026-08-03-porto-itinerary.html",
             receipt("Meridian Air — itinerary",
                     [("Outbound, seat 17C", "560.00 USD"),
                      ("Return, seat 17C", "560.00 USD")],
                     "1,120.00 USD"))
    document(date(2025, 11, 12), STUDENT_LOAN, "loan",
             "2025-11-12-student-loan-paid-in-full.html",
             receipt("Meridian Student Lending — paid in full",
                     [("Original principal", "6,240.00 USD"),
                      ("Interest over the term", "462.02 USD"),
                      ("Balance", "0.00 USD")],
                     "paid in full"))


# --------------------------------------------------------------------------
# Assembly
# --------------------------------------------------------------------------

def assertions(when):
    for account in (CHECKING, SAVINGS, BUSINESS, CARD, STORE_CARD):
        amount = d(BOOK.balance(account))
        BOOK.add(when, f"{when} balance {account:<32}{amount:>12} USD", sort=0)


def build():
    opening()
    for y, m in MONTHS:
        housing(y, m)
        subscriptions(y, m)
        car_loan(y, m)
        everyday_spend(y, m)
        discretionary(y, m)
        income(y, m)
        quarterly(y, m)
        investing(y, m)
        one_offs(y, m)
        pay_cards(y, m)
        # None of these draw on the shared generator, so they come last
        # and leave every other month exactly as it was.
        student_loan(y, m)
        furniture(y, m)
        car_value(y, m)
        store_card(y, m)
        refunds(y, m)
        if (y, m) in ((2024, 12), (2025, 12)):
            assertions(date(y + 1, 1, 1))
    paperwork()


HEADER_MAIN = '''\
;; A ledger with the shape of a lived-in one: two years of it, a business
;; and a household sharing a chart of accounts, a portfolio with a cost
;; basis, and the loose ends a real ledger always has.
;;
;; Everything in it is invented. The merchants, the clients, the tickers
;; and every amount are made up, and no part of it is taken from anyone's
;; actual books. It exists so the app can be shown, and so the reports
;; have something to report on.
;;
;;     cargo run -- examples/realistic/main.beancount
;;
;; Deliberately in here, because a report with nothing to find looks the
;; same as a report that is broken:
;;
;;   - subscriptions on a stable cadence, two of which raise their price
;;     mid-run, and one nobody has cancelled
;;   - power bills that swing with the season
;;   - a renovation tagged across five months and four categories
;;   - three trips, two of them charged in euro: one settled later, one
;;     whose euros are still on the card
;;   - a car loan secured on a car the ledger marks down every quarter,
;;     a student loan paid off and closed, a sofa on 0% credit whose rate
;;     and due day only its open directive can say, and a store card
;;     that ends every month higher than the last
;;   - a refund that lands after the statement is paid, leaving a card in
;;     credit
;;   - a portfolio bought with a recorded cost, plus staking rewards and
;;     an airdrop that arrived with none
;;   - gold that stopped being quoted in mid-2025 and is still held
;;   - a token no market ever priced
;;   - a handful of `!` charges nobody has looked at yet

option "title" "Realistic Example"
option "operating_currency" "USD"

;; Regenerate with: python3 examples/realistic/build.py

include "accounts.beancount"
include "prices.beancount"
include "2024.beancount"
include "2025.beancount"
include "2026.beancount"
'''

HEADER_ACCOUNTS = '''\
;; What the commodities are, and what every account is called.
;;
;; `name:` is the label the UI shows. `ynab:` overrides where an account
;; sits — budget, tracking or hidden — for the cases the currency alone
;; does not settle. `income:` says whether money from a source arrives
;; without work, which is the difference between a FIRE number and a
;; wish. On a liability, `rate:` (a yearly percentage), `due:` (a day of
;; the month), `limit:` and `collateral:` (an asset account) say what
;; the postings cannot.
'''

HEADER_PRICES = '''\
;; Month-end quotes.
;;
;; Gold stops here in June 2025 and is still held afterwards, on purpose:
;; a position priced three years ago is worse than one never priced at
;; all, because it still looks like a number. The mesh token is never
;; quoted at all.
'''


def render():
    os.makedirs(OUT, exist_ok=True)
    # Clear what a previous run left, so moving the dates does not silt the
    # directory up with files nothing includes any more.
    for stale in glob.glob(os.path.join(OUT, "*.beancount")):
        os.remove(stale)
    if os.path.isdir(os.path.join(OUT, "documents")):
        shutil.rmtree(os.path.join(OUT, "documents"))

    with open(os.path.join(OUT, "main.beancount"), "w") as fh:
        fh.write(HEADER_MAIN)

    with open(os.path.join(OUT, "accounts.beancount"), "w") as fh:
        fh.write(HEADER_ACCOUNTS + "\n")
        for symbol, name, klass in COMMODITIES:
            fh.write(f"2023-12-01 commodity {symbol}\n")
            fh.write(f'  name: "{name}"\n')
            fh.write(f'  asset-class: "{klass}"\n')
        fh.write("\n")
        for account, currencies, name, meta in ACCOUNTS:
            fh.write(f"2023-12-01 open {account:<34}{currencies}\n")
            if name:
                fh.write(f'  name: "{name}"\n')
            for key, value in meta:
                shown = value if isinstance(value, (int, Decimal)) else f'"{value}"'
                fh.write(f"  {key}: {shown}\n")
        fh.write("\n")
        for account, name in EXPENSES:
            fh.write(f"2023-12-01 open {account:<34}USD\n")
            fh.write(f'  name: "{name}"\n')

    with open(os.path.join(OUT, "prices.beancount"), "w") as fh:
        fh.write(HEADER_PRICES + "\n")
        for y, m in MONTHS:
            when = min(month_end(y, m), LAST)
            for symbol in ("VSTX", "VINX", "VEMX", "VBDX", "VREX", "GLDT",
                           "BTC", "ETH", "EUR"):
                if symbol == "GLDT" and (y, m) > GOLD_LAST_QUOTE:
                    continue
                quote = PRICES[symbol][(y, m)]
                fh.write(f"{when} price {symbol:<6}{quote:>12} USD\n")
            fh.write("\n")

    by_year = defaultdict(list)
    for when, sort, text in sorted(BOOK.entries, key=lambda e: (e[0], e[1])):
        by_year[when.year].append(text)
    for year, texts in sorted(by_year.items()):
        with open(os.path.join(OUT, f"{year}.beancount"), "w") as fh:
            fh.write(f";; {year}\n\n")
            fh.write("\n\n".join(texts) + "\n")

    for rel, body in DOCUMENTS:
        path = os.path.join(OUT, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as fh:
            fh.write(body)


if __name__ == "__main__":
    build()
    render()
    total = sum(1 for _ in BOOK.entries)
    print(f"{total} directives, {len(DOCUMENTS)} documents")
    for account in (CHECKING, SAVINGS, BUSINESS, BROKER, CARD, STORE_CARD,
                    CAR_LOAN, STUDENT_LOAN, FURNITURE, TRAVEL_CARD):
        low = BOOK.lows.get((account, "USD"), Decimal(0))
        inflow, outflow = BOOK.flows[account]
        n = len(MONTHS)
        print(f"  {account:<30}{d(BOOK.balance(account)):>12} USD"
              f"  low {d(low):>11}  in/mo {d(inflow / n):>10}"
              f"  out/mo {d(outflow / n):>11}")
