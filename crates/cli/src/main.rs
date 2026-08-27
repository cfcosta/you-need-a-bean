use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use bean_cli::api::{AppState, router};
use bean_cli::args::{Args, USAGE, UsageError};
use bean_cli::ui::UiSource;
use bean_cli::watch;
use bean_core::loader::load;
use bean_core::model::Ledger;
use miette::{Diagnostic, MietteHandlerOpts, Result};
use thiserror::Error;

fn main() -> ExitCode {
    install_report_handler();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(report) => {
            // `{:?}` on a report is the drawn diagnostic, not a struct dump.
            // Returning the report from `main` would draw the same thing
            // behind an `Error:` prefix that adds nothing.
            eprintln!("{report:?}");
            ExitCode::FAILURE
        }
    }
}

/// Diagnostics draw themselves; this decides how much they draw.
///
/// Two lines either side of the one that failed: a beancount error is usually
/// a posting, and the transaction above it is what makes it readable.
fn install_report_handler() {
    // Fails only if a handler is already installed, and then it is not ours
    // to replace.
    let _ = miette::set_hook(Box::new(|_| {
        Box::new(MietteHandlerOpts::new().context_lines(2).build())
    }));
}

fn run() -> Result<()> {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        // `--help` was asked for. Answering it is a success, and the answer
        // belongs on stdout rather than under a red cross on stderr.
        Err(UsageError::Help) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };

    let start = Instant::now();
    let loaded = load(&args.ledger)?;
    let ledger = Ledger::build(loaded);
    let parse_ms = start.elapsed().as_millis() as u64;

    for warning in &ledger.warnings {
        eprintln!("warning: {warning}");
    }
    println!(
        "serving http://{}:{} ({} directives across {} files in {}ms)",
        args.host,
        args.port,
        ledger.directives,
        ledger.files.len(),
        parse_ms
    );

    let mut state = AppState::new(ledger, parse_ms);
    if let Some(dir) = args.ui_dir.clone() {
        state = state.with_ui(UiSource::Dir(dir));
    }
    serve(args, Arc::new(state))
}

/// Why the server could not be run. Neither has a place in a file to point
/// at, so neither carries source: a diagnostic is a message and a remedy
/// before it is a drawing.
#[derive(Debug, Error, Diagnostic)]
enum ServeError {
    #[error("cannot listen on {host}:{port}")]
    #[diagnostic(
        code(bean::listen),
        help("something else may already hold the port; try --port")
    )]
    Listen {
        host: String,
        port: u16,
        #[source]
        source: std::io::Error,
    },

    #[error("the server stopped")]
    #[diagnostic(code(bean::serve))]
    Stopped {
        #[source]
        source: std::io::Error,
    },
}

#[tokio::main]
async fn serve(args: Args, state: Arc<AppState>) -> Result<()> {
    let listener =
        tokio::net::TcpListener::bind((args.host.as_str(), args.port))
            .await
            .map_err(|source| ServeError::Listen {
                host: args.host.clone(),
                port: args.port,
                source,
            })?;
    tokio::spawn(watch::follow(
        args.ledger.clone(),
        Arc::clone(&state),
        watch::POLL,
    ));
    axum::serve(listener, router(state))
        .await
        .map_err(|source| ServeError::Stopped { source })?;
    Ok(())
}
