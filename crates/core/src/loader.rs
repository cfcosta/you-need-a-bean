//! Reads a root beancount file and everything it transitively includes.
//!
//! Semantics follow beancount: `include` paths resolve relative to the file
//! containing the directive, glob patterns are expanded (sorted for
//! determinism), files load at most once (which also makes include cycles
//! safe), and a glob matching nothing is a warning rather than an error.

use std::collections::{HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, OnceLock};

use beancount_parser::{
    Directive, DirectiveContent, Entry, Include, parse_iter,
};
use miette::{Diagnostic, NamedSource, SourceSpan};
use rust_decimal::Decimal;
use thiserror::Error;

/// A `document` directive, with the file it names made findable.
///
/// The path is written relative to the file that declared it, and once the
/// directives are one flat list that file is gone — so resolving it is the
/// loader's job, the same as an `include`. The resolved form travels beside
/// the directives rather than overwriting them: `Document::path` on the
/// directive still says what the ledger says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Account the file is attached to.
    pub account: String,
    /// `(year, month, day)`, as [`crate::model::Day`].
    pub date: (u16, u8, u8),
    /// Absolute. The file it names may not exist; that is not this layer's
    /// question, and a document that has gone missing is worth reporting
    /// rather than quietly dropping.
    pub path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: u32,
}

/// Everything read from disk, before any budget modelling.
#[derive(Debug)]
pub struct LoadedLedger {
    /// Canonical paths in load order; the root file is first.
    pub files: Vec<PathBuf>,
    /// `option` directives as (name, value), in load order.
    pub options: Vec<(String, String)>,
    /// All directives from all files.
    pub directives: Vec<Directive<Decimal>>,
    pub origins: Vec<SourceLocation>,
    pub plugins: Vec<(String, SourceLocation)>,
    /// Every `document` directive, with its path resolved.
    pub documents: Vec<Document>,
    /// Non-fatal problems (e.g. globs that matched nothing).
    pub warnings: Vec<String>,
}

impl LoadedLedger {
    /// First value declared for an option, matching beancount's precedence.
    pub fn option(&self, name: &str) -> Option<&str> {
        self.options
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    pub fn title(&self) -> Option<&str> {
        self.option("title")
    }

    /// All `operating_currency` values, deduplicated, in declaration order.
    pub fn operating_currencies(&self) -> Vec<&str> {
        let mut seen = HashSet::new();
        self.options
            .iter()
            .filter(|(n, _)| n == "operating_currency")
            .map(|(_, v)| v.as_str())
            .filter(|v| seen.insert(*v))
            .collect()
    }
}

/// Why a ledger could not be loaded.
///
/// Every variant that *has* a location carries one, as the source text plus a
/// span into it, so `miette` can print the offending line with the rest of the
/// file around it. A ledger is hundreds of files deep; "cannot read
/// /some/absolute/path" without the line that asked for it is not an answer.
#[derive(Debug, Error, Diagnostic)]
pub enum LoadError {
    /// The root file could not be read. Nothing pointed at it, so there is no
    /// line to show.
    #[error("cannot read {path}")]
    #[diagnostic(
        code(bean::unreadable),
        help("check the path and that the file is readable")
    )]
    Unreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },

    /// A file named by an `include` could not be read.
    #[error("cannot read {path}")]
    #[diagnostic(
        code(bean::unreadable_include),
        help("include paths resolve relative to the file they are written in")
    )]
    UnreadableInclude {
        path: String,
        #[source]
        source: std::io::Error,
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label("included from here")]
        span: SourceSpan,
    },

    /// An `include` glob that is not a valid pattern.
    #[error("invalid glob pattern")]
    #[diagnostic(
        code(bean::bad_glob),
        help("`*` matches within one path segment, `**` matches across them")
    )]
    BadGlob {
        #[source]
        source: glob::PatternError,
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label("{source}")]
        span: SourceSpan,
    },

    /// A line the parser could not read.
    ///
    /// The parser reads on past an error, so a ledger with several reports
    /// them all: the first here, the rest as `others`, each with its own
    /// source and label.
    #[error("invalid beancount syntax")]
    #[diagnostic(code(bean::syntax))]
    Syntax {
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label("{label}")]
        span: SourceSpan,
        /// What the parser wanted where it stopped, and what it found.
        label: String,
        #[help]
        help: Option<String>,
        #[related]
        others: Vec<LoadError>,
        /// Errors past the ones shown. A file that is not beancount at all
        /// has one on every line, and a screenful of them says no more than
        /// a count does.
        omitted: usize,
    },
}

/// Syntax errors drawn in full; past this many, they are only counted.
const SHOWN_SYNTAX_ERRORS: usize = 10;

/// What the parser expects at the start of a line nothing claims.
const LINE_START: &str = "a directive, option, include, plugin or comment";

impl LoadError {
    /// Path of the file the error is *in*, as it would be shown to a reader.
    ///
    /// For an unreadable include that is the file holding the `include`, not
    /// the file it failed to reach — the one is where to look, the other is
    /// what to look for, and only the first is a place.
    pub fn path(&self) -> &str {
        match self {
            LoadError::Unreadable { path, .. } => path,
            LoadError::UnreadableInclude { src, .. }
            | LoadError::BadGlob { src, .. }
            | LoadError::Syntax { src, .. } => src.name(),
        }
    }

    /// Line and column the error points at, both one-based.
    ///
    /// `None` for an error with nowhere to point — a root file that could not
    /// be opened has no line to blame.
    pub fn location(&self) -> Option<(usize, usize)> {
        let src = self.source_code()?;
        let span = self.labels()?.next()?;
        let contents = src.read_span(span.inner(), 0, 0).ok()?;
        Some((contents.line() + 1, contents.column() + 1))
    }

    /// The error as a single line, location included.
    ///
    /// For the places that cannot draw a diagnostic and still have to say
    /// something useful: the API's error banner, a log. Anything with a
    /// terminal should render the diagnostic itself and get the source line
    /// with it.
    pub fn summary(&self) -> String {
        let mut summary = match self.location() {
            Some((line, column)) => {
                format!("{}:{line}:{column}: {self}", self.path())
            }
            None => format!("{}: {self}", self.path()),
        };
        if let LoadError::Syntax {
            label,
            others,
            omitted,
            ..
        } = self
        {
            summary.push_str(": ");
            summary.push_str(label);
            match others.len() + omitted {
                0 => {}
                1 => summary.push_str(" (and 1 more syntax error)"),
                more => summary
                    .push_str(&format!(" (and {more} more syntax errors)")),
            }
        }
        summary
    }

    /// Every syntax error found, as one error: the first, carrying the rest.
    fn syntax(mut errors: Vec<LoadError>) -> LoadError {
        let omitted = errors.len().saturating_sub(SHOWN_SYNTAX_ERRORS);
        errors.truncate(SHOWN_SYNTAX_ERRORS);
        let mut errors = errors.into_iter();
        let mut first = errors.next().expect("at least one syntax error");
        if let LoadError::Syntax {
            help,
            others,
            omitted: hidden,
            ..
        } = &mut first
        {
            *others = errors.collect();
            *hidden = omitted;
            if omitted > 0 {
                let note =
                    format!("{omitted} more syntax errors are not shown");
                *help = Some(match help.take() {
                    Some(help) => format!("{help}\n{note}"),
                    None => note,
                });
            }
        }
        first
    }
}

/// A file being read, kept together so an error can quote it.
///
/// Built once per file. The text is copied only when something actually goes
/// wrong, so the happy path never pays for it, and then only once however many
/// errors the file holds.
struct Source<'a> {
    path: &'a Path,
    text: &'a str,
    named: OnceLock<Arc<NamedSource<String>>>,
}

impl<'a> Source<'a> {
    fn new(path: &'a Path, text: &'a str) -> Self {
        Self {
            path,
            text,
            named: OnceLock::new(),
        }
    }

    fn named(&self) -> Arc<NamedSource<String>> {
        self.named
            .get_or_init(|| {
                Arc::new(NamedSource::new(
                    display_path(self.path),
                    self.text.to_owned(),
                ))
            })
            .clone()
    }

    /// The span an `include` directive's quoted path occupies.
    fn span_of(&self, include: &Include) -> SourceSpan {
        (include.offset, include.length).into()
    }

    /// A parser error as a diagnostic: the token the parser stopped at,
    /// underlined, labelled with what it wanted there instead.
    ///
    /// The underline covers the one token the parser could not get past. At
    /// the end of a line or the file there is no token, and the underline is
    /// one character wide so it still has somewhere to be.
    fn syntax(&self, err: &beancount_parser::Error) -> LoadError {
        let len = self.text.len();
        let offset = err.offset().min(len);
        let rest = &self.text[offset..];
        // The rule that failed says how much it read, when it read a token
        // whole. Otherwise the best guess is up to the next whitespace.
        let end = err
            .token_len()
            .filter(|&n| rest.is_char_boundary(n.min(rest.len())))
            .unwrap_or_else(|| {
                rest.find(char::is_whitespace).unwrap_or(rest.len())
            })
            .min(rest.len());
        let token = &rest[..end];
        let found = match rest.chars().next() {
            None => "the end of the file".to_string(),
            Some('\n' | '\r') => "the end of the line".to_string(),
            Some(_) if token.is_empty() => "a space".to_string(),
            Some(_) => format!("`{}`", shorten(token)),
        };
        let span: SourceSpan = if token.is_empty() {
            let at = if offset < len {
                offset
            } else {
                len.saturating_sub(1)
            };
            (at, 1).into()
        } else {
            (offset, token.len()).into()
        };
        let label = match err.expected() {
            Some(expected) => format!("expected {expected}, found {found}"),
            None => format!("unexpected {found}"),
        };
        let help = (err.expected() == Some(LINE_START)).then(|| {
            "a line that is not blank and does not start a directive is an \
             error; comments begin with `;`"
                .to_string()
        });
        LoadError::Syntax {
            src: self.named(),
            span,
            label,
            help,
            others: Vec::new(),
            omitted: 0,
        }
    }
}

/// A token short enough to quote in a label.
fn shorten(token: &str) -> String {
    const MAX: usize = 32;
    match token.char_indices().nth(MAX) {
        Some((end, _)) => format!("{}…", &token[..end]),
        None => token.to_string(),
    }
}

/// A document path as written, made absolute against the file that wrote it.
///
/// Beancount resolves these relative to the declaring file, so `../` in an
/// included file counts from *that* file's directory. `canonicalize` would
/// be the obvious tool and is the wrong one: it fails on a path naming a
/// file nobody put there, and those are exactly the ones still worth
/// showing. The directory is already canonical, so resolving `..`
/// lexically cannot disagree with what the filesystem would say.
fn resolve_document(dir: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        return path.to_path_buf();
    }
    let mut out = PathBuf::new();
    for part in dir.join(path).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            part => out.push(part),
        }
    }
    out
}

/// Absolute paths are how the loader thinks and the worst way to read a
/// diagnostic. Show one relative to the working directory when it is below it.
fn display_path(path: &Path) -> String {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| path.strip_prefix(cwd).ok())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn has_glob_chars(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Load `root` and every file it transitively includes.
pub fn load(root: &Path) -> Result<LoadedLedger, LoadError> {
    let mut ledger = LoadedLedger {
        files: Vec::new(),
        options: Vec::new(),
        directives: Vec::new(),
        origins: Vec::new(),
        plugins: Vec::new(),
        documents: Vec::new(),
        warnings: Vec::new(),
    };
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();
    let mut syntax: Vec<LoadError> = Vec::new();

    let root = root
        .canonicalize()
        .map_err(|source| LoadError::Unreadable {
            path: display_path(root),
            source,
        })?;
    seen.insert(root.clone());
    queue.push_back(root);

    while let Some(path) = queue.pop_front() {
        let text = std::fs::read_to_string(&path).map_err(|source| {
            LoadError::Unreadable {
                path: display_path(&path),
                source,
            }
        })?;
        let dir = path.parent().expect("a readable file has a parent");
        let source = Source::new(&path, &text);

        for entry in parse_iter::<Decimal>(&text) {
            let entry = match entry {
                Ok(entry) => entry,
                // Read on: the parser has resumed past it, and the rest of
                // the file -- and whatever it includes -- may hold more.
                Err(err) => {
                    syntax.push(source.syntax(&err));
                    continue;
                }
            };
            match entry {
                Entry::Directive(directive) => {
                    if let DirectiveContent::Document(doc) = &directive.content
                    {
                        ledger.documents.push(Document {
                            account: doc.account.as_str().to_owned(),
                            date: (
                                directive.date.year,
                                directive.date.month,
                                directive.date.day,
                            ),
                            path: resolve_document(dir, &doc.path),
                        });
                    }
                    ledger.origins.push(SourceLocation {
                        path: path.clone(),
                        line: directive.line_number,
                    });
                    ledger.directives.push(directive)
                }
                Entry::Option(option) => {
                    ledger.options.push((option.name, option.value))
                }
                Entry::Include(include) => {
                    enqueue_include(
                        dir,
                        &include,
                        &source,
                        &mut seen,
                        &mut queue,
                        &mut ledger.warnings,
                    )?;
                }
                Entry::Plugin(plugin) => {
                    let prior = ledger
                        .plugins
                        .iter()
                        .filter(|(name, source)| {
                            name == &plugin.name && source.path == path
                        })
                        .count();
                    let line = text
                        .lines()
                        .enumerate()
                        .filter(|(_, line)| {
                            line.trim_start().starts_with("plugin ")
                                && line.contains(&plugin.name)
                        })
                        .nth(prior)
                        .map(|(n, _)| n as u32 + 1)
                        .unwrap_or(1);
                    ledger.plugins.push((
                        plugin.name,
                        SourceLocation {
                            path: path.clone(),
                            line,
                        },
                    ))
                }
                _ => {}
            }
        }
        ledger.files.push(path);
    }

    if !syntax.is_empty() {
        return Err(LoadError::syntax(syntax));
    }
    Ok(ledger)
}

/// `source` is the file the `include` was written in, so a failure can point
/// at the line rather than only naming the file it could not reach.
fn enqueue_include(
    dir: &Path,
    include: &Include,
    source: &Source<'_>,
    seen: &mut HashSet<PathBuf>,
    queue: &mut VecDeque<PathBuf>,
    warnings: &mut Vec<String>,
) -> Result<(), LoadError> {
    let resolved = if include.path.is_relative() {
        dir.join(&include.path)
    } else {
        include.path.clone()
    };

    if has_glob_chars(&resolved.to_string_lossy()) {
        let pattern = resolved.to_string_lossy().into_owned();
        let matches =
            glob::glob(&pattern).map_err(|err| LoadError::BadGlob {
                source: err,
                src: source.named(),
                span: source.span_of(include),
            })?;
        let mut paths: Vec<PathBuf> = matches.filter_map(Result::ok).collect();
        if paths.is_empty() {
            warnings.push(format!(
                "{}:{}: include \"{}\" matched no files",
                display_path(source.path),
                include.line_number,
                include.path.display()
            ));
            return Ok(());
        }
        paths.sort();
        for path in paths {
            push_canonical(path, include, source, seen, queue)?;
        }
    } else {
        push_canonical(resolved, include, source, seen, queue)?;
    }
    Ok(())
}

fn push_canonical(
    path: PathBuf,
    include: &Include,
    source: &Source<'_>,
    seen: &mut HashSet<PathBuf>,
    queue: &mut VecDeque<PathBuf>,
) -> Result<(), LoadError> {
    let canonical =
        path.canonicalize()
            .map_err(|err| LoadError::UnreadableInclude {
                path: display_path(&path),
                source: err,
                src: source.named(),
                span: source.span_of(include),
            })?;
    if seen.insert(canonical.clone()) {
        queue.push_back(canonical);
    }
    Ok(())
}
