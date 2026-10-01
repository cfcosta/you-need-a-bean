# the parser contract

The app reads beancount through the vendored `beancount-parser` crate in
`crates/beancount-parser` (upstream 2.6.0 plus the patches in
`crates/beancount-parser/VENDOR.md`). It is a nom recursive-descent parser
carrying a set of local fixes and a handful of behaviours nobody chose.

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
plugin "beancount.plugins.auto" "config"
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
error line=12 expected="the end of the line"
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
- **Every directive ends the same way.** Its tags, then its links, then its
  metadata, whatever kind it is. Beancount allows `#tag` and `^link` on any
  directive, not just a transaction, so the dump renders them uniformly.
- **Errors say what was expected.** An error emits `error line=N
  expected="…"`, naming what the parser wanted where it got furthest, or
  bare `error line=N` when no rule had anything specific to say. The phrase
  is part of the contract: it is what the user reads.
- **Errors do not stop the parse.** After one, parsing resumes at the next
  line starting in column 1. The broken entry's own line and every indented
  or blank line under it are skipped, since they belong to it. A dump can
  hold any number of errors. → `errors/parsing-resumes-after-an-error`

`without_line_numbers` strips the `line=N` fields, for the properties where
inserting blank lines legitimately shifts everything down.

## Three layers

### 1. `corpus/` — goldens

`.beancount` files paired with the `.expected` dump they must produce, checked
by `tests/corpus.rs`. Organised by what they cover:

| directory | what is in it |
| --- | --- |
| `directives/` | one case per directive kind, plus flags and tags |
| `amounts/` | numbers, currencies, accounts, costs, prices, arithmetic |
| `strings/` | quoting, escapes, where strings are allowed |
| `structure/` | whitespace, comments, EOF, `plugin`, `pushmeta`, org-mode lines |
| `errors/` | every input that must be reported rather than skipped |
| `quirks/` | permissive behaviours kept on purpose |
| `real/` | ledgers copied from beancount's own examples |

`errors/` and `quirks/` are the two to read first. `errors/` is where the
parser's silence used to live: each case is an input that once vanished
without a word and now names its line. `quirks/` is what is left — behaviours
that are more permissive than beancount but not *wrong*, each explaining
itself in its own comments.

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

Generation over the model (`src/generate.rs`), driven by
[hegeltest](https://crates.io/crates/hegeltest):

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

hegeltest shrinks a failure to a minimal ledger and records it under
`crates/conformance/.hegel/` (gitignored) so the next run replays it first.
Each property notes the *source text* on failure rather than a `Debug` dump,
so a counterexample can be pasted straight into `corpus/` and become a
permanent case.

Shrinking is the reason for the framework rather than a hand-rolled PRNG. The
first hegeltest run found a division-by-zero panic in the parser — `1 / 0` in
a ledger aborted the process — that a uniform random generator would
essentially never hit, because it requires drawing exactly zero. Boundary
probing is what a real shrinker does and a seeded PRNG does not.

`src/draw.rs` holds the one trait both drivers implement: hegeltest's
`TestCase` for the properties, and a 30-line splitmix64 (`draw::Seeded`) for
the benchmarks, which need a fixed seed to produce a fixed input and so cannot
use a generator whose answers depend on search state. One generation body
serves both. Its two conventions matter: `chance` is written so the smallest
draw means *false*, and `pick` returns the first element for the smallest
draw, so shrinking removes optional features and the constant tables are
ordered simplest-first.

`tests/grammar.rs` holds the assertions that read better with a reason
attached than as a golden: precedence, absent-vs-empty, API equivalence,
limits, hostile input.

## Behaviours a rewrite must reproduce (or change on purpose)

Every one of these has a case in `corpus/`.

**Reported, not skipped**

The parser used to end in a catch-all that discarded any line no rule claimed,
so the ledger came out short with no error and no warning. Each of these is
now a syntax error naming its line, and each has a case pinning that.

- A posting line the parser cannot read — and every posting after it in the
  same transaction. → `errors/unparseable-posting`, `errors/unclosed-cost`
- A line matching no rule at all. → `errors/junk-line`
- A top-level keyword the parser does not know. →
  `errors/unknown-top-level-keyword`
- `#` or `^` with nothing after it. → `errors/bare-tag-marker`

The line beancount *does* ignore still is: one opening in column 1 with any of
`*:#!&?%`, so a ledger can double as an org-mode document. →
`structure/org-mode-lines`

**Everything beancount defines is modelled**

`note`, `document`, `query` and `custom` produce directives;
`plugin`, `pushmeta` and `popmeta` are read at the top level. This is a
prerequisite for the section above rather than a feature: a strict catch-all
is only safe once the parser knows the whole language. → `directives/note`,
`directives/document`, `directives/query`, `directives/custom`,
`structure/plugin`, `structure/pushmeta`

Tags and links are read on *any* directive, not just a transaction. →
`directives/tags-on-any-directive`

`pushmeta` nests: each key holds a stack, the innermost push wins, and a key
written on the directive itself beats anything pushed.

**Strings**

- Only `\"` and `\\` are escapes; `\n` is a syntax error, not a newline. →
  `errors/unknown-string-escape`
- An escape is valid anywhere, including at the start of a string, twice in a
  row, or as the entire contents. → `strings/escapes`

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
- Division by zero is a syntax error, not a panic — including when it is
  reached through arithmetic, as in `1 / (2 - 2)`. → `errors/division-by-zero`,
  `amounts/division`

**Collections**

- Duplicate metadata keys collapse to the last value. →
  `quirks/duplicate-metadata-keys`
- Duplicate tags and links collapse. The pushtag stack is a set, so pushing
  twice and popping once removes the tag; popping an unpushed tag is a no-op.
  → `quirks/pushtag-duplicates`, `quirks/poptag-without-pushtag`

**Structure**

- A capitalised word after a date matches the *flag* rule, so
  `2026-01-01 Open Assets:Cash` is a hard syntax error. →
  `errors/capital-keyword`
- A directive or a comment on the last line without a trailing newline is
  fine. → `structure/no-trailing-newline`,
  `structure/comment-at-eof-without-newline`

**Enough location to point at**

The dump contract stops at `error line=N expected="…"`, but the application needs more than
a line. `crates/core` draws the offending source with the failure underlined,
and a line number alone cannot say which part of the line to underline, or
which `include` in a large include graph asked for a file that is not there. Two
pieces of API carry that, and a rewrite that drops them turns every diagnostic
back into a sentence:

- `Error::offset()` — the byte the parse got furthest to: the start of the
  token no rule could read, not the start of its entry. The loader underlines
  that token, which is a presentation decision the parser has no answer to.
- `Error::token_len()` — how much of the input from that byte the failing
  rule read before rejecting it, when it read a whole token: the `13` of a
  month. The loader underlines exactly that, or guesses up to the next
  whitespace when it is `None`.
- `Error::expected()` — the same phrase the dump prints. The loader puts it in
  the label under the underline, beside what it found there instead.
- `Include { path, line_number, offset, length }` on `Entry::Include` — where
  the directive was *written*, not just where it points.

Neither is observable in the dump, so neither is pinned by a corpus case;
`crates/core/tests/loader.rs` pins them instead, by asserting on the text each
diagnostic underlines.

**Limits**

- Nesting recurses one stack frame per level with no depth limit, so deep
  enough input aborts the process. Measured on a 2 MiB thread: ~212 levels in
  a debug build, ~1650 in release (`cargo run -p you-need-a-bean-conformance
  --example depth` re-measures it). The suite pins a floor of 20 and leaves
  the ceiling to the implementation. → `tests/grammar.rs`

What remains in `quirks/` is permissiveness, not error. Range-checked dates,
commas anywhere, a space after a sign, a posting without an amount, last-write
-wins metadata: beancount would reject some of these, but rejecting them is a
*validation* decision that belongs above the parser, where the message can say
what was expected. The unbounded recursion is the one genuine defect still
pinned, and it is pinned so a rewrite notices it rather than to argue for it.

## Checking against real ledgers

The corpus pins behaviour on inputs someone thought to write down. This
answers the other question — whether the parser handles the ledger someone
actually has:

```sh
cargo run -p you-need-a-bean-conformance --example check -- examples ~/some-ledger
```

One line per file: the count of each entry kind, or the line the parse stopped
on. Run it before and after a rewrite and compare — the totals should be
identical and no file should regress into an error. It is how the patches
above were validated, and how many silently-dropped `document` directives
in a large validation ledger were found.

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
| `pathological` | one huge transaction, mostly comments, long strings, deep expressions, a deep `pushmeta` stack, an error at the end — the shapes where accidental quadratic behaviour would show |

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
5. Only then swap the parser path in `[workspace.dependencies]` and run the
   rest of the workspace.
