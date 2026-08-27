//! Hand-rolled argument parsing: one positional ledger path plus a few
//! flags is not worth a dependency.

use std::path::PathBuf;

use miette::Diagnostic;
use thiserror::Error;

pub const USAGE: &str = "usage: you-need-a-bean <LEDGER> [--host HOST] \
[--port PORT] [--ui-dir DIR]";

/// Why a command line could not be turned into [`Args`].
///
/// Every variant answers with the usage line, because every way of writing
/// the command line wrong has the same remedy. `Help` is not a failure, but
/// it ends parsing the same way one does; the caller tells them apart.
#[derive(Debug, Error, Diagnostic)]
pub enum UsageError {
    #[error("no ledger to serve")]
    #[diagnostic(code(bean::args::no_ledger), help("{}", USAGE))]
    NoLedger,

    #[error("unknown flag: {flag}")]
    #[diagnostic(code(bean::args::unknown_flag), help("{}", USAGE))]
    UnknownFlag { flag: String },

    #[error("unexpected argument: {arg}")]
    #[diagnostic(code(bean::args::unexpected), help("{}", USAGE))]
    Unexpected { arg: String },

    #[error("{flag} needs a value")]
    #[diagnostic(code(bean::args::missing_value), help("{}", USAGE))]
    MissingValue { flag: String },

    #[error("invalid port: {raw}")]
    #[diagnostic(
        code(bean::args::invalid_port),
        help("a port is a number from 0 to 65535")
    )]
    InvalidPort {
        raw: String,
        #[source]
        source: std::num::ParseIntError,
    },

    /// `--help`. Asked for, so not an error, but there is nothing left to do.
    #[error("{}", USAGE)]
    #[diagnostic(code(bean::args::help))]
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub ledger: PathBuf,
    pub host: String,
    pub port: u16,
    /// Serve UI assets from disk instead of the embedded bundle.
    pub ui_dir: Option<PathBuf>,
}

impl Args {
    pub fn parse(
        args: impl IntoIterator<Item = String>,
    ) -> Result<Self, UsageError> {
        let mut ledger = None;
        let mut host = "127.0.0.1".to_string();
        let mut port = 2326;
        let mut ui_dir = None;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(UsageError::Help),
                "--host" => host = value(&mut args, "--host")?,
                "--port" => {
                    let raw = value(&mut args, "--port")?;
                    port = raw.parse().map_err(|source| {
                        UsageError::InvalidPort { raw, source }
                    })?;
                }
                "--ui-dir" => {
                    ui_dir = Some(PathBuf::from(value(&mut args, "--ui-dir")?));
                }
                _ if arg.starts_with('-') => {
                    return Err(UsageError::UnknownFlag { flag: arg });
                }
                _ if ledger.is_some() => {
                    return Err(UsageError::Unexpected { arg });
                }
                _ => ledger = Some(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            ledger: ledger.ok_or(UsageError::NoLedger)?,
            host,
            port,
            ui_dir,
        })
    }
}

fn value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, UsageError> {
    args.next().ok_or_else(|| UsageError::MissingValue {
        flag: flag.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, UsageError> {
        Args::parse(args.iter().map(|s| s.to_string()))
    }

    /// The help text, which is where the usage line now lives: the message
    /// says what is wrong, the help says what to write instead.
    fn help(err: &UsageError) -> String {
        err.help().map(|h| h.to_string()).unwrap_or_default()
    }

    #[test]
    fn parses_ledger_with_defaults() {
        let args = parse(&["books/main.beancount"]).unwrap();
        assert_eq!(args.ledger, PathBuf::from("books/main.beancount"));
        assert_eq!(args.host, "127.0.0.1");
        assert_eq!(args.port, 2326);
        assert_eq!(args.ui_dir, None);
    }

    #[test]
    fn parses_flags_in_any_order() {
        let args = parse(&[
            "--port",
            "8080",
            "main.beancount",
            "--host",
            "0.0.0.0",
            "--ui-dir",
            "ui/dist",
        ])
        .unwrap();
        assert_eq!(args.ledger, PathBuf::from("main.beancount"));
        assert_eq!(args.host, "0.0.0.0");
        assert_eq!(args.port, 8080);
        assert_eq!(args.ui_dir, Some(PathBuf::from("ui/dist")));
    }

    #[test]
    fn rejects_bad_invocations() {
        let err = parse(&[]).unwrap_err();
        assert!(matches!(err, UsageError::NoLedger));
        assert!(help(&err).contains("usage:"), "{}", help(&err));

        assert!(matches!(parse(&["-h"]).unwrap_err(), UsageError::Help));
        assert!(parse(&["-h"]).unwrap_err().to_string().contains("usage:"));

        let err = parse(&["a", "b"]).unwrap_err();
        assert!(matches!(err, UsageError::Unexpected { .. }));
        assert!(err.to_string().contains("unexpected argument: b"));

        let err = parse(&["--nope"]).unwrap_err();
        assert!(matches!(err, UsageError::UnknownFlag { .. }));
        assert!(err.to_string().contains("unknown flag: --nope"));

        let err = parse(&["x", "--port"]).unwrap_err();
        assert!(matches!(err, UsageError::MissingValue { .. }));
        assert!(err.to_string().contains("--port needs a value"));

        let err = parse(&["x", "--port", "99999"]).unwrap_err();
        assert!(matches!(err, UsageError::InvalidPort { .. }));
        assert!(err.to_string().contains("invalid port: 99999"));
    }

    #[test]
    fn every_way_to_get_it_wrong_says_how_to_get_it_right() {
        // A message without the usage line leaves the reader guessing, and
        // the guess is usually another wrong command line.
        for args in
            [vec![], vec!["a", "b"], vec!["--nope"], vec!["x", "--port"]]
        {
            let err = parse(&args).unwrap_err();
            assert!(help(&err).contains("usage:"), "{args:?}: {err}");
        }
    }
}
