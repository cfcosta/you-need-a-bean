# you need a bean

A local, read-only financial home for [Beancount](https://beancount.github.io/)
ledgers. See your current position, cash after reserves, upcoming commitments,
savings goals, and the evidence behind the numbers. Explore spending,
investments, and debts, or search the full ledger with Cmd/Ctrl+K.

```sh
you-need-a-bean path/to/main.beancount
# → you need a bean · serving http://127.0.0.1:2326
```

Beancount stays the source of truth: this tool never writes to your ledger.

## Features

- A responsive financial overview across the whole ledger, with account
  coverage, goals, a review queue, and a 30/60/90-day commitments forecast.
- Explicit accounting checks and source references; incomplete accounting
  and missing prices pause global planning.
- Search across payees, accounts, dates, tags, links, and metadata.
- Current figures exclude future transactions and future quotes.

- Monthly budget table with per-category "vs typical" bars and
  good / warning / over states; navigate back through any month.
- Targets are trailing averages of your own history (3, 6 or 12 months).
- Multi-currency: amounts stay native, conversion uses your `price`
  directives and per-category currency splits, displayed in the ledger’s default currency.
- Friendly account names via `name: "…"` metadata on `open` directives.
- Follows `include` directives (relative paths and globs), scales to
  ledgers with hundreds of thousands of directives.
- Single self-contained binary — the UI is embedded.

## Usage

```
you-need-a-bean <LEDGER> [--port 2326] [--host 127.0.0.1] [--ui-dir ui/dist]
```

## Development

Rust workspace (`crates/core`, `crates/cli`) plus a Bun + React + Tailwind
UI in `ui/`. `cargo build` embeds a fresh UI build automatically.

```sh
cargo nextest run          # rust tests
bun test --cwd ui          # ui unit tests
treefmt                    # formatting (nix flake formatter)
cargo run -- examples/realistic/main.beancount
```

`examples/overview.beancount` is the overview demo to open first. Regenerate
it with `python3 examples/build-overview.py --today YYYY-MM-DD`.
[Overview rules and account metadata](docs/HOME.md) explain coverage,
reserves, forecasting assumptions, and unsupported accounting.

`examples/realistic/` exercises the broader reports: two and a half years of
a household and a one-person business sharing a chart of accounts, with a
portfolio, recurring merchants, two trips and the loose ends that give
every report something to report on. All of it invented — no part of it
comes from anyone's books. `examples/realistic/build.py` regenerates it,
which is also how to move its dates forward once they age.

Design and computation rules live in `docs/PLAN.md`; `docs/mockup-v3.html`
is the layout the UI grew out of, though the look has moved on since.
