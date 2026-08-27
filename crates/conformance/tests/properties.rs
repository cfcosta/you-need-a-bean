//! Properties the parser must satisfy for every input, not just the corpus.
//!
//! The corpus pins behaviour on cases someone thought of. These pin behaviour
//! on cases nobody did: hegeltest builds ledgers through
//! `bean_conformance::generate`, checks each against the model in
//! `bean_conformance::ast`, and when one disagrees, shrinks it to the smallest
//! ledger that still disagrees before printing it. That last part is the whole
//! value — a 40-directive counterexample tells you nothing, and the three-line
//! version it shrinks to is usually a corpus case ready to paste.
//!
//! Failures are remembered in a `.hegel` directory beside the workspace, so a
//! counterexample found once is retried first on every later run.

use bean_conformance::ast::Ledger;
use bean_conformance::draw::Draw;
use bean_conformance::generate::{self, Config, Shape};
use bean_conformance::{dump, without_line_numbers};
use hegel::TestCase;

/// A ledger of a drawn shape and a drawn size.
///
/// The size is drawn rather than fixed so shrinking can make a failing file
/// shorter as well as simpler, and `max` is small on purpose: a property that
/// needs a hundred directives to fail is a property about something else.
fn ledger(tc: &TestCase, max: usize) -> Ledger {
    let mut draw = tc;
    let shape = *draw.pick(&Shape::ALL);
    of_shape(tc, shape, max)
}

fn of_shape(tc: &TestCase, shape: Shape, max: usize) -> Ledger {
    let mut draw = tc;
    let directives = 1 + draw.below(max);
    generate::ledger(&mut draw, Config::new(shape, directives))
}

/// Attach the source to the failure report. `Ledger`'s `Debug` would print the
/// tree; this prints the file, which is the form you can paste into `corpus/`.
fn note(tc: &TestCase, label: &str, text: &str) {
    tc.note(&format!("--- {label} ---\n{text}"));
}

/// The load-bearing property: parsing the model's text reproduces the model's
/// dump. Every field, every line number, every folded expression.
#[hegel::test(test_cases = 400)]
fn parsing_a_rendered_ledger_reproduces_the_model(tc: TestCase) {
    let rendered = ledger(&tc, 24).render();
    let actual = dump(&rendered.text);
    note(&tc, "source", &rendered.text);
    if actual != rendered.dump {
        note(
            &tc,
            "diff",
            &bean_conformance::corpus::diff(&rendered.dump, &actual),
        );
    }
    assert_eq!(
        rendered.dump, actual,
        "parsing the model's own text did not reproduce its dump",
    );
}

/// Re-indenting a file changes nothing but line numbers: the parser must not
/// let horizontal whitespace leak into any value.
#[hegel::test]
fn extra_horizontal_whitespace_is_not_meaningful(tc: TestCase) {
    let text = ledger(&tc, 16).render().text;
    let padded: String = text
        .lines()
        .map(|line| {
            if line.starts_with([' ', '\t']) {
                format!("   {line}   \n")
            } else {
                format!("{line}   \n")
            }
        })
        .collect();
    note(&tc, "source", &text);
    assert_eq!(
        dump(&text),
        dump(&padded),
        "padding lines changed the parse"
    );
}

/// Blank lines and comments carry no meaning, so inserting them may move
/// directives down the file but must not change anything else.
#[hegel::test]
fn blank_lines_and_comments_only_move_line_numbers(tc: TestCase) {
    let text = ledger(&tc, 16).render().text;
    let noisy: String = text
        .lines()
        .flat_map(|line| {
            // Only between top-level constructs: a blank line inside a
            // transaction is legal but a comment column-aligned with a posting
            // would be a different (also legal) thing to test.
            if line.is_empty() || line.starts_with([' ', '\t']) {
                vec![format!("{line}\n")]
            } else {
                vec![
                    "\n".to_string(),
                    "; noise\n".to_string(),
                    format!("{line}\n"),
                ]
            }
        })
        .collect();
    note(&tc, "source", &text);
    assert_eq!(
        without_line_numbers(&dump(&text)),
        without_line_numbers(&dump(&noisy)),
        "interleaving blank lines and comments changed the parse",
    );
}

/// Concatenating two files parses as the concatenation of their parses. This
/// is what makes `include` sound, and it fails loudly if the parser carries
/// state across directives that it should not.
///
/// Sound for every shape because generation closes what it opens: a ledger
/// ends with a `poptag` and a `popmeta` for everything still on its stacks, so
/// the second file starts clean however the first one was built.
#[hegel::test]
fn concatenation_is_additive(tc: TestCase) {
    let a = ledger(&tc, 8).render().text;
    let b = ledger(&tc, 8).render().text;
    let joined = format!("{a}{b}");
    let expected = format!(
        "{}{}",
        without_line_numbers(&dump(&a)),
        without_line_numbers(&dump(&b)),
    );
    note(&tc, "first", &a);
    note(&tc, "second", &b);
    assert_eq!(
        expected,
        without_line_numbers(&dump(&joined)),
        "concatenating two files did not concatenate their parses",
    );
}

/// Every prefix of a file that ends on a construct boundary parses to the
/// matching prefix of the dump: the parser must not need to see the end of the
/// file to decide what an earlier directive means.
#[hegel::test(test_cases = 50)]
fn parsing_is_incremental(tc: TestCase) {
    let whole = ledger(&tc, 12);
    for cut in 1..=whole.entries.len() {
        let prefix = Ledger {
            entries: whole.entries[..cut].to_vec(),
        };
        let rendered = prefix.render();
        note(&tc, &format!("first {cut} entries"), &rendered.text);
        assert_eq!(
            rendered.dump,
            dump(&rendered.text),
            "the first {cut} entries did not parse on their own",
        );
    }
}

/// A file must parse the same whether or not it ends with a newline. This is
/// the shape a `.beancount` file arrives in when an editor trims it.
#[hegel::test]
fn a_trailing_newline_is_optional(tc: TestCase) {
    let text = ledger(&tc, 12).render().text;
    note(&tc, "source", &text);
    assert_eq!(
        dump(&text),
        dump(text.trim_end_matches('\n')),
        "dropping the trailing newline changed the parse",
    );
}

/// Windows line endings must parse identically to Unix ones.
#[hegel::test]
fn carriage_returns_are_accepted(tc: TestCase) {
    let text = ledger(&tc, 12).render().text;
    note(&tc, "source", &text);
    assert_eq!(
        dump(&text),
        dump(&text.replace('\n', "\r\n")),
        "CRLF line endings changed the parse",
    );
}

/// Parsing must be a pure function of the input.
#[hegel::test]
fn parsing_is_deterministic(tc: TestCase) {
    let text = of_shape(&tc, Shape::Rich, 16).render().text;
    note(&tc, "source", &text);
    let first = dump(&text);
    for _ in 0..3 {
        assert_eq!(first, dump(&text), "parse is not stable");
    }
}

/// Seeded generation must be reproducible, or a benchmark number means
/// nothing. This is the one thing here that is not about the parser: it is
/// about the other half of `generate`, the half hegeltest does not drive.
#[test]
fn seeded_generation_is_reproducible() {
    for shape in Shape::ALL {
        let config = Config::new(shape, 20);
        let first = generate::seeded(1234, config).render();
        let again = generate::seeded(1234, config).render();
        assert_eq!(first.text, again.text, "{}: text differs", shape.name());
        assert_eq!(first.dump, again.dump, "{}: dump differs", shape.name());
        let other = generate::seeded(1235, config).render();
        assert_ne!(
            first.text,
            other.text,
            "{}: different seeds produced the same file",
            shape.name(),
        );
    }
}
