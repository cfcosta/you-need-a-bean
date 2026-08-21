# Vendored: beancount-parser 2.6.0

Source: <https://github.com/jcornaz/beancount-parser>, crates.io release
2.6.0, license Unlicense (see `UNLICENSE`). Wired into the build through
`[patch.crates-io]` in the workspace root `Cargo.toml`, so
`crates/core` keeps depending on plain `beancount-parser = "2.6.0"`.

## Local changes vs upstream

`src/amount.rs`: amount expressions accept a unary plus, matching Python
beancount's grammar.

- `literal` takes an optional `+` or `-` sign (upstream: `-` only).
- `negation` handles `+ (…)` as well as `- (…)`.

`src/transaction.rs`: costs accept the total form `{{ 700.00 USD }}`
alongside the per-unit form `{ 5.00 USD }`, matching Python beancount's
grammar.

- `cost` recognises `{{ … }}` delimiters.
- `Cost` gains a `total: bool` field (the struct is `#[non_exhaustive]`,
  so this is not a breaking change for downstream readers).

Upstream silently drops posting lines it cannot parse (the transaction's
posting iterator stops, and the leftover lines match the top-level
comment fallback), so without these patches every `+123.45 USD`-style
or `{{ … }}`-cost posting vanished from the ledger along with the
postings after it in the same transaction — no error, just wrong
balances.

Everything else is byte-identical to the crates.io release. `vendor/` is
excluded from treefmt so the diff against upstream stays reviewable.
Drop this directory and the `[patch.crates-io]` entry once the fix is
released upstream.
