//! Hand-rolled argument parsing: one positional ledger path plus a few
//! flags is not worth a dependency.

use std::path::PathBuf;

pub const USAGE: &str = "usage: you-need-a-bean <LEDGER> [--host HOST] \
[--port PORT] [--ui-dir DIR]";

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
    ) -> Result<Self, String> {
        let mut ledger = None;
        let mut host = "127.0.0.1".to_string();
        let mut port = 2326;
        let mut ui_dir = None;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => return Err(USAGE.to_string()),
                "--host" => host = value(&mut args, "--host")?,
                "--port" => {
                    let raw = value(&mut args, "--port")?;
                    port = raw
                        .parse()
                        .map_err(|_| format!("invalid port: {raw}"))?;
                }
                "--ui-dir" => {
                    ui_dir = Some(PathBuf::from(value(&mut args, "--ui-dir")?));
                }
                _ if arg.starts_with('-') => {
                    return Err(format!("unknown flag: {arg}\n{USAGE}"));
                }
                _ if ledger.is_some() => {
                    return Err(format!("unexpected argument: {arg}\n{USAGE}"));
                }
                _ => ledger = Some(PathBuf::from(arg)),
            }
        }

        Ok(Self {
            ledger: ledger.ok_or_else(|| USAGE.to_string())?,
            host,
            port,
            ui_dir,
        })
    }
}

fn value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("{flag} needs a value\n{USAGE}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        Args::parse(args.iter().map(|s| s.to_string()))
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
        assert!(parse(&[]).unwrap_err().contains("usage:"));
        assert!(parse(&["-h"]).unwrap_err().contains("usage:"));
        assert!(parse(&["a", "b"]).unwrap_err().contains("unexpected"));
        assert!(parse(&["--nope"]).unwrap_err().contains("unknown flag"));
        assert!(
            parse(&["x", "--port"])
                .unwrap_err()
                .contains("needs a value")
        );
        assert!(
            parse(&["x", "--port", "99999"])
                .unwrap_err()
                .contains("invalid port")
        );
    }
}
