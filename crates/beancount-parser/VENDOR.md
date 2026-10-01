# Vendored: beancount-parser 2.6.0

Source: <https://github.com/jcornaz/beancount-parser>, crates.io release
2.6.0, license Unlicense (see `UNLICENSE`). The crate is a workspace member
and consumers use the path dependency declared in `[workspace.dependencies]`.

Everything not listed below is byte-identical to the crates.io release.
`crates/beancount-parser/` is excluded from treefmt so the diff against
upstream stays reviewable — do not reformat it. Every patch
carries a `Local patch vs upstream 2.6.0` comment at its site pointing
back here. Replace the workspace dependency with the crates.io release once
the fixes are released upstream.

Every change here is pinned by a case in `crates/conformance/corpus/`,
so a rewrite of the parser has to reproduce it. See `docs/PARSER.md`.

## The theme: silence

Upstream's top-level `entry` parser ended in a catch-all that matched and
discarded *any* line the rules above did not claim. A mistyped directive,
a posting the transaction parser gave up on, a `document` directive it did
not model, plain prose — all of it produced no entry, no error and no
warning. The ledger just came out short, and the only symptom was a
balance that did not add up.

Most of what follows is one fix seen from different angles: teach the
parser the syntax beancount actually defines, then make the catch-all
strict so anything left over is reported.

## Directives upstream did not model

`src/extra.rs` (new file) adds `note`, `document`, `query`, `custom`, and
the top-level `plugin`, `pushmeta` and `popmeta`. `src/lib.rs` gains
`DirectiveContent::{Note, Document, Query, Custom}` with the matching
`as_*` accessors, `Entry::Plugin`, `BeancountFile::plugins`, and
`RawEntry::{Plugin, PushMeta, PopMeta}`.

These are not exotic. A large validation ledger contained many `document`
directives, all of which vanished before this.

`src/iterator.rs` grows a `meta_stack` alongside the existing `tag_stack`
and applies it the same way: `pushmeta`/`popmeta` nest, each key holds a
stack, the innermost push wins, and a key written on the directive itself
beats anything pushed. The `Iter` type gained a `D` parameter to hold the
pushed values.

`src/metadata.rs`: `value` is split out of `entry`, and both it and `key`
are now `pub(crate)`, so `pushmeta` parses its argument with the same code
as an ordinary metadata line.

## Tags and links on non-transaction directives

`src/lib.rs`: `Directive` gains `tags` and `links`, and `directive` reads
them for every kind. `src/transaction.rs`: `tags_and_links` is
`pub(super)`.

Beancount allows these on any directive — `#scanned` on a `document` is
routine — but upstream only parsed them after a transaction. Such a line
matched no rule and went down the catch-all, so the *whole directive*
disappeared, not just its tags. Both fields are always empty for a
transaction, whose tags absorb the pushed tag stack and so live on
`Transaction::tags`.

## The catch-all

`src/lib.rs`: `line` is replaced by `ignored_line`, which is
`empty_line` (unchanged, renamed) or the new `org_mode_line`. Anything
else is now a syntax error naming the line.

`org_mode_line` claims a line opening in column 1 with one of `*:#!&?%`,
which is what beancount itself skips so that a ledger can double as an
org-mode document.

This is the change that turns every silent drop above into a report, and
it is why the rest of the patches exist: making the catch-all strict is
only safe once the parser knows the whole language.

## Empty tags and links

`src/transaction.rs`: `parse_tag` and `parse_link` use `take_while1`
rather than `take_while`. A bare `#` or `^` is a syntax error instead of a
tag with an empty name.

## Strings

`src/lib.rs`: `string` loops until the escape parser stops matching,
rather than until a run of literal characters comes back empty. Upstream's
condition let an escape parse only when preceded by at least one ordinary
character and never two in a row, so `"\"quoted\""`, `"a\\\\b"` and
`"\n"` were all syntax errors.

## Division by zero

`src/amount.rs`: `product` rejects a zero divisor instead of dividing.

`rust_decimal`'s `Div` panics, so `1 / 0` in a ledger — or anything that
evaluates to it, like `1 / (2 - 2)` — took the whole process down rather
than reporting a bad line. It is a syntax error now for every `D`, so the
behaviour does not depend on which decimal type the caller picked. Found
by the property tests in `crates/conformance` within seconds of pointing
a shrinking generator at the parser.

## Grammar upstream did not accept

`src/amount.rs`: expressions accept a unary plus, matching Python
beancount. `literal` takes an optional `+` or `-` (upstream: `-` only),
and `negation` handles `+ (…)` as well as `- (…)`.

`src/transaction.rs`: costs accept the total form `{{ 700.00 USD }}`
alongside the per-unit `{ 5.00 USD }`. `Cost` gains a `total: bool`
field; the struct is `#[non_exhaustive]`, so this is not a breaking change
for downstream readers.

Both were the original reason for vendoring: every `+123.45 USD`-style
or `{{ … }}`-cost posting used to vanish along with the postings after it
in the same transaction.

## Where the error is

The two patches here are not about silence but about location: upstream
says *that* a parse failed and roughly where, but not precisely enough
for a caller to point at it.

`src/error.rs`: `Error` carries a byte `offset` unconditionally, read
back through `Error::offset()`. Upstream keeps that offset only under the
`miette` feature, folded into a `SourceSpan` inside a `Diagnostic` it
derives itself — which fixes the presentation (miette 5, a zero-length
span, no code, no help) and makes the error's *shape* depend on a feature
flag. The raw offset instead lets `crates/core` build whatever diagnostic
it likes, and decide how far past the offset to underline: the parser
knows where it stopped, not how much is wrong.

`src/lib.rs`: `Entry::Include` and `RawEntry::Include` carry an `Include`
struct — the path as written, the line the directive sits on, and the
byte span of the quoted path — where upstream had a bare `PathBuf`. A
ledger can be hundreds of includes deep, and `cannot read /some/abs/path`
says nothing about which of them asked for it. The include emitted by the
file-following reader reports the *resolved* path; where it was written
stays as written. `BeancountFile::includes` is still a `Vec<PathBuf>`.

## Edition and dependencies

`Cargo.toml`: edition 2024 and `rust-version = "1.98"`, the stable toolchain the repo builds with, (upstream: 2021
and 1.68), `miette` 7 (upstream: 5) and `rstest` 0.27 (upstream: 0.26).
`cargo fix --edition` needed no source changes.

Upstream's 1.68 floor held the whole workspace back: Cargo's resolver
picks dependency versions that satisfy the lowest `rust-version` of any
member, so a plain `cargo update` left dozens of crates behind. miette 5
also pulled a second copy of `thiserror` (1.x) into the build.

## Clippy

`src/amount.rs`: `currency` uses `is_none_or` where upstream had
`map_or(true, …)`; same behaviour.

`src/lib.rs`: `Entry` and `RawEntry` allow `clippy::large_enum_variant`.
`Directive` is far bigger than the other variants, but nearly every
entry the parser yields is a directive, so boxing it would add a heap
allocation per entry, and change the public `Entry` type, to save space
on the rare `option`, `include` and `plugin` lines.
