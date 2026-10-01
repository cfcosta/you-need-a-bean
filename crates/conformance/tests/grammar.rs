//! Behaviour that is clearer as an assertion with a reason attached than as a
//! file in the corpus: the shape of the public API, the limits of the
//! grammar, and the handful of invariants the dump format itself must hold.

use bean_conformance::dump;
use beancount_parser::{
    BeancountFile, Directive, DirectiveContent, Entry, parse, parse_iter,
};
use rust_decimal::Decimal;

/// Parse and assert on the dump. Most tests here are one input, one answer.
fn d(input: &str) -> String {
    dump(input)
}

/// The one line of the dump that starts with `needle`, for tests that care
/// about a single field.
fn field<'a>(dumped: &'a str, needle: &str) -> &'a str {
    dumped
        .lines()
        .map(str::trim_start)
        .find(|line| line.starts_with(needle))
        .unwrap_or("<missing>")
}

// ---------------------------------------------------------------------------
// The two entry points must agree
// ---------------------------------------------------------------------------

/// `parse` collects, `parse_iter` streams. A rewrite is free to implement one
/// in terms of the other, but they must never disagree — `bean_core::loader`
/// uses the streaming one and everything else assumes the collected one.
#[test]
fn parse_and_parse_iter_agree() {
    let inputs = [
        include_str!("../corpus/real/basic.beancount"),
        include_str!("../corpus/real/starter.beancount"),
        include_str!("../corpus/real/vesting.beancount"),
        include_str!("../corpus/directives/transaction.beancount"),
        include_str!("../corpus/amounts/expressions.beancount"),
    ];
    for input in inputs {
        let collected: BeancountFile<Decimal> =
            parse(input).expect("corpus file parses");
        let streamed: Vec<Entry<Decimal>> = parse_iter(input)
            .collect::<Result<_, _>>()
            .expect("corpus file streams");

        let from_iter: Vec<&Directive<Decimal>> =
            streamed.iter().filter_map(Entry::as_directive).collect();
        assert_eq!(
            collected.directives.len(),
            from_iter.len(),
            "the two entry points found different numbers of directives",
        );
        for (a, b) in collected.directives.iter().zip(from_iter) {
            assert_eq!(a.date, b.date);
            assert_eq!(a.line_number, b.line_number);
        }
    }
}

/// Both entry points agree on the first error. `parse` stops there; the
/// streaming iterator reports it at the same place and carries on.
#[test]
fn both_entry_points_report_the_same_first_error() {
    let input = "2026-01-01 open Assets:Cash\n2026-13-01 open Assets:Bad\n";
    let err = parse::<Decimal>(input).expect_err("month 13 is invalid");
    let streamed: Vec<_> = parse_iter::<Decimal>(input).collect();
    let before = streamed.iter().take_while(|e| e.is_ok()).count();
    assert_eq!(before, 1, "the good directive should come through first");
    let first = streamed
        .iter()
        .find_map(|e| e.as_ref().err())
        .expect("an error entry");
    assert_eq!(
        err.line_number(),
        first.line_number(),
        "the two entry points reported the error on different lines",
    );
}

/// The streaming iterator reports every error in the input, each once, with
/// the entries between them intact.
#[test]
fn the_iterator_reports_every_error() {
    let input = "2026-13-01 open Assets:A\n\
                 2026-01-01 open Assets:B\n\
                 this is not beancount\n\
                 2026-01-02 open Assets:C\n\
                 2026-01-03 open Assets:D Opps\n";
    let lines: Vec<Result<u32, u32>> = parse_iter::<Decimal>(input)
        .map(|entry| match entry {
            Ok(Entry::Directive(d)) => Ok(d.line_number),
            Ok(other) => panic!("unexpected entry {other:?}"),
            Err(err) => Err(err.line_number()),
        })
        .collect();
    assert_eq!(lines, vec![Err(1), Ok(2), Err(3), Ok(4), Err(5)]);
}

/// An error says what the parser was looking for where it stopped, and where
/// that was: the furthest point any rule reached, not the start of the entry.
#[test]
fn an_error_names_what_was_expected_and_where() {
    let input = "2026-01-01 * \"x\"\n  Assets:Cash  -42.50 US$\n";
    let err = parse::<Decimal>(input).expect_err("`US$` is not a currency");
    assert_eq!(err.line_number(), 2);
    assert_eq!(err.expected(), Some("the end of the line"));
    assert_eq!(&input[err.offset()..], "$\n");
}

// ---------------------------------------------------------------------------
// Line numbers
// ---------------------------------------------------------------------------

/// Line numbers are 1-based and point at the directive's *first* line, not
/// its last. Every error message the app shows a user depends on this.
#[test]
fn line_numbers_are_one_based_and_point_at_the_header() {
    let dumped = d("\n\n2026-01-15 * \"x\"\n  Assets:Cash 1 USD\n");
    assert!(
        dumped.starts_with("directive line=3 "),
        "expected the header line, got:\n{dumped}",
    );
}

/// Metadata and postings do not shift the next directive's line number.
#[test]
fn line_numbers_survive_multi_line_directives() {
    let dumped = d("2026-01-01 * \"a\"\n  k: \"v\"\n  Assets:Cash 1 USD\n\
                    2026-01-02 * \"b\"\n  Assets:Cash 1 USD\n");
    let lines: Vec<&str> = dumped
        .lines()
        .filter(|l| l.starts_with("directive "))
        .collect();
    assert_eq!(lines[0], "directive line=1 2026-01-01 txn");
    assert_eq!(lines[1], "directive line=4 2026-01-02 txn");
}

// ---------------------------------------------------------------------------
// Numbers
// ---------------------------------------------------------------------------

/// Trailing zeros are part of the value, not noise: `1.50 USD` and `1.5 USD`
/// must stay distinguishable, because reports print what the ledger wrote.
#[test]
fn decimal_scale_is_preserved() {
    assert_eq!(
        field(&d("2026-01-01 price A 1.50 USD\n"), "amount"),
        "amount 1.50 USD"
    );
    assert_eq!(
        field(&d("2026-01-01 price A 1.5 USD\n"), "amount"),
        "amount 1.5 USD"
    );
    assert_eq!(
        field(&d("2026-01-01 price A 1 USD\n"), "amount"),
        "amount 1 USD"
    );
}

/// Arithmetic folds left, and `*`/`/` bind tighter than `+`/`-`. Getting the
/// precedence wrong changes numbers silently, which is the worst kind of bug
/// a ledger can have.
#[test]
fn arithmetic_precedence_and_associativity() {
    let cases = [
        ("2 + 3 * 4", "14"),
        ("(2 + 3) * 4", "20"),
        ("2 * 3 + 4", "10"),
        ("100 - 10 - 10", "80"),
        ("100 / 10 / 10", "1"),
        ("1 - 2 + 3", "2"),
        // 6 / 4: rust_decimal picks the scale, so this is 1.50 not 1.5.
        ("2 * 3 / 4", "1.50"),
        ("-(1 + 2)", "-3"),
    ];
    for (expr, expected) in cases {
        let dumped = d(&format!("2026-01-01 price A {expr} USD\n"));
        assert_eq!(
            field(&dumped, "amount"),
            format!("amount {expected} USD"),
            "`{expr}` did not evaluate to {expected}",
        );
    }
}

/// A unary sign only applies to a parenthesised group. A bare `-5` is a
/// signed literal, which is a different rule reaching the same answer — but
/// `- (5)` and `-5` must both work.
#[test]
fn unary_signs() {
    for (expr, expected) in [
        ("-5", "-5"),
        ("+5", "5"),
        ("-(5)", "-5"),
        ("+(5)", "5"),
        ("- (5)", "-5"),
        ("-  5", "-5"),
    ] {
        let dumped = d(&format!("2026-01-01 price A {expr} USD\n"));
        assert_eq!(
            field(&dumped, "amount"),
            format!("amount {expected} USD"),
            "`{expr}` did not evaluate to {expected}",
        );
    }
}

/// Division is exact where it can be and rounds where it cannot; either way
/// it must be deterministic, because a report that changes between runs is
/// worse than one that is slightly wrong.
#[test]
fn division_is_deterministic() {
    let dumped = d("2026-01-01 price A 1 / 3 USD\n");
    let first = field(&dumped, "amount").to_string();
    for _ in 0..5 {
        assert_eq!(
            field(&d("2026-01-01 price A 1 / 3 USD\n"), "amount"),
            first
        );
    }
    assert!(first.starts_with("amount 0.3333"), "got {first}");
}

// ---------------------------------------------------------------------------
// Absent versus empty
// ---------------------------------------------------------------------------

/// An empty narration is not a missing narration, and a rewrite that
/// conflates them loses information a user typed on purpose.
#[test]
fn empty_is_not_absent() {
    let empty = d("2026-01-01 * \"\"\n  Assets:Cash 1 USD\n");
    let absent = d("2026-01-01 *\n  Assets:Cash 1 USD\n");
    assert!(empty.contains("narration \"\""), "got:\n{empty}");
    assert!(!absent.contains("narration"), "got:\n{absent}");
    assert_ne!(empty, absent);
}

/// `txn` means no flag; it is not a flag spelled `txn`.
#[test]
fn the_txn_keyword_is_the_absence_of_a_flag() {
    let keyword = d("2026-01-01 txn \"x\"\n  Assets:Cash 1 USD\n");
    let starred = d("2026-01-01 * \"x\"\n  Assets:Cash 1 USD\n");
    assert!(!keyword.contains("flag"), "got:\n{keyword}");
    assert!(starred.contains("flag *"), "got:\n{starred}");
}

/// A cost of `{}` is present-and-empty, distinct from no cost at all.
#[test]
fn an_empty_cost_is_still_a_cost() {
    let with = d("2026-01-01 * \"x\"\n  Assets:Cash 1 USD {}\n");
    let without = d("2026-01-01 * \"x\"\n  Assets:Cash 1 USD\n");
    assert!(with.contains("cost unit\n"), "got:\n{with}");
    assert!(!without.contains("cost"), "got:\n{without}");
}

// ---------------------------------------------------------------------------
// Canonical ordering
// ---------------------------------------------------------------------------

/// Tags, links, `open` currencies and metadata all live in hash containers,
/// so the dump sorts them. Without that, the goldens would be a coin flip.
#[test]
fn unordered_collections_dump_in_sorted_order() {
    let one = d("2026-01-01 * \"x\" #b #a #c ^z ^y\n  Assets:Cash 1 USD\n");
    let other = d("2026-01-01 * \"x\" #c #b #a ^y ^z\n  Assets:Cash 1 USD\n");
    assert_eq!(one, other, "tag order in the source leaked into the dump");
    let tags: Vec<&str> = one
        .lines()
        .filter(|l| l.trim_start().starts_with("tag "))
        .collect();
    assert_eq!(tags, ["  tag a", "  tag b", "  tag c"]);

    let currencies = d("2026-01-01 open Assets:X CHF,USD,EUR\n");
    assert_eq!(currencies, d("2026-01-01 open Assets:X USD,EUR,CHF\n"));
}

/// Running the dump twice on the same input gives the same bytes. The whole
/// golden corpus rests on this.
#[test]
fn the_dump_is_stable_across_runs() {
    let input = include_str!("../corpus/real/basic.beancount");
    let first = d(input);
    for _ in 0..5 {
        assert_eq!(first, d(input));
    }
}

// ---------------------------------------------------------------------------
// Structure and limits
// ---------------------------------------------------------------------------

/// Nesting recurses one stack frame per level, with no depth limit, so deep
/// enough input aborts the process rather than returning an error. That is a
/// real limit of this parser and a rewrite should fix it — but until then the
/// contract is a floor, not a ceiling: nesting up to `MAX_DEPTH` must work.
///
/// The measured ceiling on a 2 MiB thread is ~212 levels in a debug build and
/// ~1650 in a release build (`cargo run --example depth` re-measures it), so
/// the floor sits an order of magnitude below the tighter of the two.
const MAX_DEPTH: usize = 20;

#[test]
fn nesting_up_to_the_documented_depth_works() {
    for depth in 1..=MAX_DEPTH {
        let expr = format!("{}1 + 1{}", "(".repeat(depth), ")".repeat(depth));
        let dumped = d(&format!("2026-01-01 price A {expr} USD\n"));
        assert_eq!(
            field(&dumped, "amount"),
            "amount 2 USD",
            "nesting {depth} deep did not parse",
        );
    }
}

/// Unbalanced brackets are an error, not a hang.
#[test]
fn unbalanced_parentheses_are_an_error() {
    for expr in ["(1", "1)", "((1)", "(1))", "()"] {
        let dumped = d(&format!("2026-01-01 price A {expr} USD\n"));
        assert!(
            dumped.starts_with("error"),
            "`{expr}` should not have parsed:\n{dumped}",
        );
    }
}

/// A transaction with many postings and a file with many directives must both
/// parse; this is a smoke test for anything quadratic.
#[test]
fn wide_inputs_parse() {
    let mut wide = String::from("2026-01-01 * \"many legs\"\n");
    for i in 0..2000 {
        wide.push_str(&format!("  Assets:Cash  {i} USD\n"));
    }
    assert_eq!(d(&wide).lines().filter(|l| *l == "  posting").count(), 2000);

    let mut tall = String::new();
    for i in 0..2000 {
        tall.push_str(&format!("2026-01-01 price A{} 1 USD\n", i % 26 + 65));
    }
    assert_eq!(
        d(&tall)
            .lines()
            .filter(|l| l.starts_with("directive "))
            .count(),
        2000,
    );
}

/// Input that is not a ledger at all must produce an error or nothing, never
/// a panic. Users paste the wrong file into the wrong place.
#[test]
fn hostile_input_does_not_panic() {
    let cases: &[&str] = &[
        "",
        "\0",
        "\u{feff}2026-01-01 open Assets:Cash\n",
        "\"",
        "{{{{{{{{",
        "((((((((((",
        "2026-01-01",
        "2026-01-01 ",
        "2026-01-01 *",
        "2026-01-01 * \"",
        "2026-01-01 price ",
        "-",
        "#",
        "  ",
        "\r\n\r\n",
        "2026-01-01 open Assets:\u{202e}Cash\n",
        "𝔘𝔫𝔦𝔠𝔬𝔡𝔢",
    ];
    for case in cases {
        // The assertion is that this returns at all.
        let _ = d(case);
    }
}

/// A very long single line must not be treated specially.
#[test]
fn a_long_line_parses() {
    let narration = "x".repeat(100_000);
    let dumped = d(&format!(
        "2026-01-01 * \"{narration}\"\n  Assets:Cash 1 USD\n"
    ));
    assert!(
        dumped.contains(&narration),
        "the long narration was truncated"
    );
}

// ---------------------------------------------------------------------------
// The typed API, for the parts `bean_core` reaches into
// ---------------------------------------------------------------------------

/// `bean_core::loader` matches on `DirectiveContent` and reads
/// `Directive::line_number`; a rewrite must keep both meaningful even if the
/// types change shape.
#[test]
fn the_typed_api_exposes_what_the_loader_needs() {
    let input = "option \"title\" \"T\"\n\
                 include \"other.beancount\"\n\
                 2026-01-15 * \"x\"\n  Assets:Cash 1 USD\n";
    let entries: Vec<Entry<Decimal>> =
        parse_iter(input).collect::<Result<_, _>>().expect("parses");

    let option = entries
        .iter()
        .find_map(Entry::as_option)
        .expect("an option");
    assert_eq!(
        (option.name.as_str(), option.value.as_str()),
        ("title", "T")
    );

    let include = entries
        .iter()
        .find_map(Entry::as_include)
        .expect("an include");
    assert_eq!(include.to_string_lossy(), "other.beancount");

    let directive = entries
        .iter()
        .find_map(Entry::as_directive)
        .expect("a directive");
    assert_eq!(directive.line_number, 3);
    let DirectiveContent::Transaction(txn) = &directive.content else {
        panic!("expected a transaction, got {:?}", directive.content);
    };
    assert_eq!(txn.postings.len(), 1);
    assert_eq!(txn.postings[0].account.as_str(), "Assets:Cash");
}

/// Errors carry a line number, which is the only thing the CLI shows a user.
#[test]
fn errors_report_a_line_number() {
    let err = parse::<Decimal>(
        "2026-01-01 open Assets:Cash\n\n2026-13-01 open Assets:X\n",
    )
    .expect_err("month 13 is invalid");
    assert_eq!(err.line_number(), 3);
    assert!(
        !err.to_string().is_empty(),
        "an error with an empty message is useless in the CLI",
    );
}
