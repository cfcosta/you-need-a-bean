# the parser contract

The app reads beancount through the vendored `beancount-parser` crate in
`vendor/beancount-parser` (upstream 2.6.0 plus the patches in
`vendor/beancount-parser/VENDOR.md`). It is a nom recursive-descent parser
carrying two local fixes and a handful of behaviours nobody chose.

`crates/conformance` exists so that crate can be replaced. It pins what the
parser does — not how — in enough detail that a from-scratch rewrite can be
declared finished when the suite is green.

## The pin

Everything rests on one function:

```rust
bean_conformance::dump(input: &str) -> String
```

It parses `input` and renders everything the parse observed as deterministic,
line-oriented text:

```text
option "operating_currency" "USD"
include "months/2026-01.beancount"
directive line=7 2026-01-15 txn
  flag *
  payee "Whole Foods"
  narration "groceries"
  tag food
  link inv-1
  meta rate number 4.55
  posting
    account Assets:Cash
    amount -10.00 CHF
    cost unit amount=2 PLN date=2026-01-01
    price total 25.00 EUR
    meta lot string "A"
error line=12
```

Deliberately, the contract is this text and nothing else. It says nothing
about types, module layout, error types or which combinator library is used —
so a rewrite is free to be a hand-written lexer, a PEG, or anything else, and
still be checkable line by line.

What the format guarantees:

- **Sorted, not hashed.** Tags, links, `open` currencies and metadata keys
  live in `HashSet`/`HashMap`, so the dump sorts them by string. Source order
  is not observable and must not be.
- **Absent is not empty.** A field that is present emits its line; a field
  that is absent emits nothing. `narration ""` and no narration line are
  different parses, because they are different ledgers.
- **Scale is data.** Numbers print through `rust_decimal::Decimal`, so
  `1.50 USD` and `1.5 USD` stay distinct.
- **Line numbers are part of the answer.** Every directive carries the line
  its header sits on. The CLI shows them to users.
- **Errors stop the parse.** The first error emits `error line=N` and nothing
  follows.

`without_line_numbers` strips the `line=N` fields, for the properties where
inserting blank lines legitimately shifts everything down.

## Three layers

### 1. `corpus/` — goldens

`.beancount` files paired with the `.expected` dump they must produce, checked
by `tests/corpus.rs`. Organised by what they cover: `directives/`, `amounts/`,
`strings/`, `structure/`, `quirks/`, `errors/`, `real/`.

`quirks/` is the part to read first. Each file explains, in its own comments,
a behaviour that is surprising and load-bearing.

To accept a deliberate behaviour change:

```sh
BLESS=1 cargo test -p you-need-a-bean-conformance --test corpus
git diff crates/conformance/corpus     # this diff *is* the behaviour change
```

Review that diff. A golden accepted without reading it is worse than no test.

### 2. `src/ast.rs` — the reference model

An independent beancount AST. `Ledger::render()` makes one pass and emits both
the source text and the dump that parsing it must produce. Nothing in it calls
the parser, so `dump(rendered.text) == rendered.dump` is a genuine
two-implementation comparison rather than a tautology.

### 3. `tests/properties.rs` — randomized differential testing

Seeded generation over the model (`src/generate.rs`, `src/rng.rs`), thousands
of ledgers per run:

| property | what it would catch |
| --- | --- |
| `parsing_a_rendered_ledger_reproduces_the_model` | any dropped, mangled or mis-folded field |
| `extra_horizontal_whitespace_is_not_meaningful` | whitespace leaking into a value |
| `blank_lines_and_comments_only_move_line_numbers` | comments changing a parse |
| `concatenation_is_additive` | state carried across directives — the thing that makes `include` unsound |
| `parsing_is_incremental` | a directive whose meaning depends on what follows it |
| `a_trailing_newline_is_optional` | EOF handling |
| `carriage_returns_are_accepted` | CRLF files |
| `parsing_is_deterministic` | hash-order leaking into the result |

Failures shrink (`generate::minimize`) and print a seed plus the smallest
ledger that still disagrees; paste it into `corpus/` and it becomes a
permanent case.

The RNG is a 30-line splitmix64 in `src/rng.rs` rather than a property-testing
framework. That keeps `cargo test` hermetic and dependency-free — the shrinker
is a hundred lines and the seeds reproduce anywhere, which is the part that
actually matters.

`tests/grammar.rs` holds the assertions that read better with a reason
attached than as a golden: precedence, absent-vs-empty, API equivalence,
limits, hostile input.

## Behaviours a rewrite must reproduce (or change on purpose)

Every one of these has a case in `corpus/quirks/` or `corpus/errors/`.

**Silent data loss**

- An unparseable posting line is *skipped*, and so is every posting after it
  in the same transaction. The transaction survives with fewer legs, which
  silently changes balances. `bean_core::loader::dropped_posting_warning`
  exists solely to notice this. → `quirks/dropped-postings`,
  `quirks/unclosed-cost-drops-postings`
- A line matching no rule at all is skipped without a warning. Only a line
  that *starts* like a directive and then goes wrong is an error. →
  `quirks/junk-lines-are-skipped`
- `note`, `document`, `query` and `custom` are not modelled, so they vanish
  entirely — no entry, no error. → `quirks/unmodelled-keywords`

**Strings**

- Only `\"` and `\\` are escapes; `\n` is a syntax error, not a newline. →
  `errors/unknown-string-escape`
- **A defect:** an escape works only when at least one ordinary character
  precedes it. `"\"quoted\""` and `"a\\\\b"` are syntax errors, because the
  loop in `string()` stops as soon as a literal run comes back empty. →
  `quirks/escapes-need-a-literal-prefix`, `errors/escape-at-string-start`,
  `errors/consecutive-escapes`

**Numbers and dates**

- `2026-02-30` and `2025-02-29` parse: month and day are range-checked, never
  calendar-checked. → `quirks/dates`
- A metadata value written as a date is an *expression*: `2026-01-01` is
  2026 − 1 − 1 = 2024. → `quirks/metadata-dates-are-arithmetic`
- `,` is stripped from a literal wherever it appears, so `1,0,0` is 100 and
  `,100` is 100. → `quirks/commas-anywhere`
- Whitespace is allowed between a sign and its digits: `- 5` is −5. →
  `quirks/space-after-sign`
- `*` and `/` bind tighter than `+` and `-`; both fold left. A unary sign
  applies only to a parenthesised group. → `amounts/expressions`

**Collections**

- Duplicate metadata keys collapse to the last value. →
  `quirks/duplicate-metadata-keys`
- Duplicate tags and links collapse. The pushtag stack is a set, so pushing
  twice and popping once removes the tag; popping an unpushed tag is a no-op.
  → `quirks/pushtag-duplicates`, `quirks/poptag-without-pushtag`
- `#` and `^` with nothing after them are an empty tag and an empty link, not
  errors. → `quirks/empty-tags`

**Structure**

- A capitalised word after a date matches the *flag* rule, so
  `2026-01-01 Open Assets:Cash` is a hard syntax error. →
  `errors/capital-keyword`
- A directive on the last line without a trailing newline is fine; a
  *comment* there is a syntax error, because the fallback rule needs a line
  ending. → `errors/comment-at-eof-without-newline`,
  `structure/no-trailing-newline`

**Limits**

- Nesting recurses one stack frame per level with no depth limit, so deep
  enough input aborts the process. Measured on a 2 MiB thread: ~212 levels in
  a debug build, ~1650 in release (`cargo run -p you-need-a-bean-conformance
  --example depth` re-measures it). The suite pins a floor of 20 and leaves
  the ceiling to the implementation. → `tests/grammar.rs`

Several of these are defects rather than decisions — the string-escape bug and
the unbounded recursion in particular. They are pinned so a rewrite *notices*
them, not to argue they should be preserved. Fixing one means re-blessing its
golden with an explanation in the commit message.

## Benchmarks

Two suites, because they answer different questions.

`crates/conformance/benches/parse.rs` — the parser alone, on strings in
memory:

| group | question |
| --- | --- |
| `throughput` | bytes per second at 1k / 10k / 100k directives |
| `shape` | which kind of content is slow — plain, rich, arithmetic, metadata, non-transaction directives |
| `api` | `parse` vs `parse_iter` vs the conformance `dump` |
| `real` | the ledgers in `examples/` |
| `pathological` | one huge transaction, mostly comments, long strings, deep expressions, constant posting drops, an error at the end — the shapes where accidental quadratic behaviour would show |

`crates/core/benches/pipeline.rs` — what the CLI actually runs: read files,
follow includes, parse, build the model.

| group | question |
| --- | --- |
| `load` | end-to-end at 1k / 10k / 50k directives |
| `includes` | the same content behind 1 / 12 / 120 `include` directives |
| `stages` | parse vs model build, side by side |

Inputs are generated from fixed seeds, so a number measured today is
comparable to one measured next month.

```sh
cargo bench -p you-need-a-bean-conformance -- --save-baseline before
# ... change the parser ...
cargo bench -p you-need-a-bean-conformance -- --baseline before
```

`stages` is the one to read before deciding to rewrite anything: on a 20k
directive ledger, parsing is roughly two thirds of the wall clock and building
the model is the other third, so even an infinitely fast parser leaves a third
of the time on the table.

## Replacing the parser

1. Add the new crate. Point `crates/conformance/Cargo.toml` at it — that crate
   depends on the parser and `rust_decimal` and nothing else in the workspace,
   so it can be developed against in isolation.
2. Rewrite `src/dump.rs` against the new API. It is the only file that touches
   the parser; the corpus, the model and the properties do not change.
3. `cargo test -p you-need-a-bean-conformance`. Every remaining failure is a
   behaviour difference. Decide, case by case, whether it is a bug in the
   rewrite or an improvement worth re-blessing.
4. `cargo bench -p you-need-a-bean-conformance -- --baseline before`.
5. Only then swap `[patch.crates-io]` in the root manifest and run the rest of
   the workspace.
