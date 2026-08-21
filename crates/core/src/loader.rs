//! Reads a root beancount file and everything it transitively includes.
//!
//! Semantics follow beancount: `include` paths resolve relative to the file
//! containing the directive, glob patterns are expanded (sorted for
//! determinism), files load at most once (which also makes include cycles
//! safe), and a glob matching nothing is a warning rather than an error.

use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::path::{Path, PathBuf};

use beancount_parser::{Directive, Entry, parse_iter};
use rust_decimal::Decimal;

/// Everything read from disk, before any budget modelling.
#[derive(Debug)]
pub struct LoadedLedger {
    /// Canonical paths in load order; the root file is first.
    pub files: Vec<PathBuf>,
    /// `option` directives as (name, value), in load order.
    pub options: Vec<(String, String)>,
    /// All directives from all files.
    pub directives: Vec<Directive<Decimal>>,
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

#[derive(Debug)]
pub enum LoadError {
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Syntax {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, source } => {
                write!(f, "cannot read {}: {source}", path.display())
            }
            LoadError::Syntax { path, message } => {
                write!(f, "syntax error in {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

fn has_glob_chars(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

/// Load `root` and every file it transitively includes.
pub fn load(root: &Path) -> Result<LoadedLedger, LoadError> {
    let mut ledger = LoadedLedger {
        files: Vec::new(),
        options: Vec::new(),
        directives: Vec::new(),
        warnings: Vec::new(),
    };
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut queue: VecDeque<PathBuf> = VecDeque::new();

    let root = root.canonicalize().map_err(|source| LoadError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    seen.insert(root.clone());
    queue.push_back(root);

    while let Some(path) = queue.pop_front() {
        let text =
            std::fs::read_to_string(&path).map_err(|source| LoadError::Io {
                path: path.clone(),
                source,
            })?;
        let dir = path.parent().expect("a readable file has a parent");

        for entry in parse_iter::<Decimal>(&text) {
            let entry = entry.map_err(|err| LoadError::Syntax {
                path: path.clone(),
                message: err.to_string(),
            })?;
            match entry {
                Entry::Directive(directive) => {
                    ledger.directives.push(directive)
                }
                Entry::Option(option) => {
                    ledger.options.push((option.name, option.value))
                }
                Entry::Include(include) => {
                    enqueue_include(
                        dir,
                        &include,
                        &mut seen,
                        &mut queue,
                        &mut ledger.warnings,
                    )?;
                }
                _ => {}
            }
        }
        ledger.files.push(path);
    }

    Ok(ledger)
}

fn enqueue_include(
    dir: &Path,
    include: &Path,
    seen: &mut HashSet<PathBuf>,
    queue: &mut VecDeque<PathBuf>,
    warnings: &mut Vec<String>,
) -> Result<(), LoadError> {
    let resolved = if include.is_relative() {
        dir.join(include)
    } else {
        include.to_path_buf()
    };

    if has_glob_chars(&resolved.to_string_lossy()) {
        let pattern = resolved.to_string_lossy().into_owned();
        let matches =
            glob::glob(&pattern).map_err(|err| LoadError::Syntax {
                path: resolved.clone(),
                message: format!("invalid glob pattern: {err}"),
            })?;
        let mut paths: Vec<PathBuf> = matches.filter_map(Result::ok).collect();
        if paths.is_empty() {
            warnings.push(format!(
                "include \"{}\" matched no files",
                include.display()
            ));
            return Ok(());
        }
        paths.sort();
        for path in paths {
            push_canonical(path, seen, queue)?;
        }
    } else {
        push_canonical(resolved, seen, queue)?;
    }
    Ok(())
}

fn push_canonical(
    path: PathBuf,
    seen: &mut HashSet<PathBuf>,
    queue: &mut VecDeque<PathBuf>,
) -> Result<(), LoadError> {
    let canonical = path.canonicalize().map_err(|source| LoadError::Io {
        path: path.clone(),
        source,
    })?;
    if seen.insert(canonical.clone()) {
        queue.push_back(canonical);
    }
    Ok(())
}
