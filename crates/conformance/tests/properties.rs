//! Properties the parser must satisfy for every input, not just the corpus.
//!
//! The corpus pins behaviour on cases someone thought of. These pin behaviour
//! on cases nobody did: thousands of seeded ledgers per run, each checked
//! against the model in `bean_conformance::ast`, with failures shrunk to
//! something small enough to paste into the corpus.

use bean_conformance::ast::Ledger;
use bean_conformance::generate::{self, Config, Shape};
use bean_conformance::{dump, without_line_numbers};

/// Cases per shape. Enough to be worth running on every `cargo test` without
/// making it slow; the seed is printed on failure so any case reproduces.
const CASES: u64 = 60;

fn seeds(shape: Shape) -> impl Iterator<Item = u64> {
    // Offsetting per shape keeps the shapes from generating the same numbers.
    let offset =
        Shape::ALL.iter().position(|s| *s == shape).unwrap_or(0) as u64;
    (0..CASES).map(move |i| i * 977 + offset * 1_000_003 + 1)
}

/// The load-bearing property: parsing the model's text reproduces the model's
/// dump. Every field, every line number, every folded expression.
#[test]
fn parsing_a_rendered_ledger_reproduces_the_model() {
    for shape in Shape::ALL {
        for seed in seeds(shape) {
            let config = Config::new(shape, 24);
            let ledger = generate::ledger(seed, config);
            let rendered = ledger.render();
            if dump(&rendered.text) == rendered.dump {
                continue;
            }
            report(shape, seed, ledger);
        }
    }
}

/// Shrink and print the smallest ledger that still disagrees.
fn report(shape: Shape, seed: u64, ledger: Ledger) -> ! {
    let smallest = generate::minimize(ledger, |candidate| {
        let rendered = candidate.render();
        dump(&rendered.text) != rendered.dump
    });
    let rendered = smallest.render();
    let actual = dump(&rendered.text);
    panic!(
        "shape={} seed={seed}: parsing the model's own text did not \
         reproduce its dump\n\n--- source ---\n{}\n--- expected \
         ---\n{}\n--- actual ---\n{}\n{}",
        shape.name(),
        rendered.text,
        rendered.dump,
        actual,
        bean_conformance::corpus::diff(&rendered.dump, &actual),
    );
}

/// Re-indenting a file changes nothing but line numbers: the parser must not
/// let horizontal whitespace leak into any value.
#[test]
fn extra_horizontal_whitespace_is_not_meaningful() {
    for seed in seeds(Shape::Realistic) {
        let ledger = generate::ledger(seed, Config::new(Shape::Realistic, 16));
        let text = ledger.render().text;
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
        assert_eq!(
            dump(&text),
            dump(&padded),
            "seed={seed}: padding lines changed the parse",
        );
    }
}

/// Blank lines and comments carry no meaning, so inserting them may move
/// directives down the file but must not change anything else.
#[test]
fn blank_lines_and_comments_only_move_line_numbers() {
    for seed in seeds(Shape::Realistic) {
        let ledger = generate::ledger(seed, Config::new(Shape::Realistic, 16));
        let text = ledger.render().text;
        let noisy: String = text
            .lines()
            .flat_map(|line| {
                // Only between top-level constructs: a blank line inside a
                // transaction is legal but a comment column-aligned with a
                // posting would be a different (also legal) thing to test.
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
        assert_eq!(
            without_line_numbers(&dump(&text)),
            without_line_numbers(&dump(&noisy)),
            "seed={seed}: interleaving blank lines and comments changed the parse",
        );
    }
}

/// Concatenating two files parses as the concatenation of their parses. This
/// is what makes `include` sound, and it fails loudly if the parser carries
/// state across directives that it should not.
#[test]
fn concatenation_is_additive() {
    for seed in seeds(Shape::Plain) {
        let a = generate::ledger(seed, Config::new(Shape::Plain, 8));
        let b = generate::ledger(seed + 7, Config::new(Shape::Directives, 8));
        let (a, b) = (a.render().text, b.render().text);
        let joined = format!("{a}{b}");
        let expected = format!(
            "{}{}",
            without_line_numbers(&dump(&a)),
            without_line_numbers(&dump(&b)),
        );
        assert_eq!(
            expected,
            without_line_numbers(&dump(&joined)),
            "seed={seed}: concatenating two files did not concatenate their parses",
        );
    }
}

/// Every prefix of a file that ends on a construct boundary parses to the
/// matching prefix of the dump: the parser must not need to see the end of
/// the file to decide what an earlier directive means.
#[test]
fn parsing_is_incremental() {
    for seed in seeds(Shape::Realistic).take(20) {
        let ledger = generate::ledger(seed, Config::new(Shape::Realistic, 12));
        for cut in 1..=ledger.entries.len() {
            let prefix = Ledger {
                entries: ledger.entries[..cut].to_vec(),
            };
            let rendered = prefix.render();
            assert_eq!(
                dump(&rendered.text),
                rendered.dump,
                "seed={seed}: the first {cut} entries did not parse on their own",
            );
        }
    }
}

/// A file must parse the same whether or not it ends with a newline. This is
/// the shape a `.beancount` file arrives in when an editor trims it.
#[test]
fn a_trailing_newline_is_optional() {
    for seed in seeds(Shape::Realistic) {
        let ledger = generate::ledger(seed, Config::new(Shape::Realistic, 12));
        let text = ledger.render().text;
        let trimmed = text.trim_end_matches('\n');
        assert_eq!(
            dump(&text),
            dump(trimmed),
            "seed={seed}: dropping the trailing newline changed the parse",
        );
    }
}

/// Windows line endings must parse identically to Unix ones.
#[test]
fn carriage_returns_are_accepted() {
    for seed in seeds(Shape::Realistic).take(20) {
        let ledger = generate::ledger(seed, Config::new(Shape::Realistic, 12));
        let text = ledger.render().text;
        let crlf = text.replace('\n', "\r\n");
        assert_eq!(
            dump(&text),
            dump(&crlf),
            "seed={seed}: CRLF line endings changed the parse",
        );
    }
}

/// Parsing must be a pure function of the input.
#[test]
fn parsing_is_deterministic() {
    for seed in seeds(Shape::Rich).take(20) {
        let text = generate::ledger(seed, Config::new(Shape::Rich, 16))
            .render()
            .text;
        let first = dump(&text);
        for _ in 0..3 {
            assert_eq!(first, dump(&text), "seed={seed}: parse is not stable");
        }
    }
}

/// Generation itself must be reproducible, or a failing seed is worthless.
#[test]
fn generation_is_reproducible() {
    for shape in Shape::ALL {
        let config = Config::new(shape, 20);
        let first = generate::ledger(1234, config).render();
        let again = generate::ledger(1234, config).render();
        assert_eq!(first.text, again.text, "{}: text differs", shape.name());
        assert_eq!(first.dump, again.dump, "{}: dump differs", shape.name());
        let other = generate::ledger(1235, config).render();
        assert_ne!(
            first.text,
            other.text,
            "{}: different seeds produced the same file",
            shape.name(),
        );
    }
}
