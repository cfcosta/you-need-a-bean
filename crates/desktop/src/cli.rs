//! `you-need-a-bean <LEDGER> [--today YYYY-MM-DD]`

use std::path::PathBuf;

use bean_core::model::Day;

#[derive(Debug, PartialEq, Eq)]
pub struct Options {
    pub ledger: PathBuf,
    /// Pins "today", for screenshots and for reading the ledger as of a
    /// past day.
    pub today: Option<Day>,
}

pub const USAGE: &str =
    "usage: you-need-a-bean <LEDGER> [--today YYYY-MM-DD]";

pub fn parse(
    args: impl IntoIterator<Item = String>,
) -> Result<Options, String> {
    let mut ledger = None;
    let mut today = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--today" => {
                let value = args.next().ok_or("--today needs a date")?;
                today = Some(bean_core::home::parse_date(&value).ok_or_else(
                    || format!("--today: `{value}` is not a YYYY-MM-DD date"),
                )?);
            }
            "-h" | "--help" => return Err(USAGE.into()),
            flag if flag.starts_with('-') => {
                return Err(format!("unknown option `{flag}`\n{USAGE}"));
            }
            path if ledger.is_none() => ledger = Some(PathBuf::from(path)),
            extra => {
                return Err(format!("unexpected argument `{extra}`\n{USAGE}"));
            }
        }
    }
    Ok(Options {
        ledger: ledger.ok_or(USAGE)?,
        today,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn takes_the_ledger_path() {
        let o = parse(args(&["main.beancount"])).unwrap();
        assert_eq!(
            o,
            Options {
                ledger: "main.beancount".into(),
                today: None
            }
        );
    }

    #[test]
    fn today_can_be_pinned() {
        let o =
            parse(args(&["--today", "2026-10-01", "main.beancount"])).unwrap();
        assert_eq!(o.today, Some((2026, 10, 1)));
    }

    #[test]
    fn a_missing_ledger_or_bad_date_is_refused() {
        assert!(parse(args(&[])).is_err());
        assert!(parse(args(&["x.beancount", "--today", "October"])).is_err());
        assert!(parse(args(&["x.beancount", "--port", "1"])).is_err());
        assert!(parse(args(&["a.beancount", "b.beancount"])).is_err());
    }

    #[test]
    fn help_names_the_binary() {
        assert_eq!(
            parse(args(&["--help"])),
            Err("usage: you-need-a-bean <LEDGER> [--today YYYY-MM-DD]".into())
        );
    }
}
