# you need a bean

A read-only, YNAB-style web UI for [beancount](https://beancount.github.io/)
ledgers. Point it at your main file and it shows each month as a budget:
what you spent per category versus what you *typically* spend (a trailing
3/6/12-month average as the target), multi-currency aware, with the full
transaction detail — metadata, tags, links, postings — one click away.

```sh
you-need-a-bean path/to/main.beancount
# → you need a bean · serving http://127.0.0.1:2326
```

Beancount stays the source of truth: this tool never writes to your ledger.

## Features

- Monthly budget table with per-category "vs typical" bars and
  good / warning / over states; navigate back through any month.
- Targets are trailing averages of your own history (3, 6 or 12 months).
- Multi-currency: amounts stay native, conversion uses your `price`
  directives, per-category currency splits, operating-currency toggle.
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

`examples/realistic/` is the ledger to open first: two and a half years of
a household and a one-person business sharing a chart of accounts, with a
portfolio, recurring merchants, two trips and the loose ends that give
every report something to report on. All of it invented — no part of it
comes from anyone's books. `examples/realistic/build.py` regenerates it,
which is also how to move its dates forward once they age.

Design and computation rules live in `docs/PLAN.md`; `docs/mockup-v3.html`
is the layout the UI grew out of, though the look has moved on since.
