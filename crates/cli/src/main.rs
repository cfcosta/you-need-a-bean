use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use bean_cli::api::{AppState, router};
use bean_cli::args::Args;
use bean_core::loader::load;
use bean_core::model::Ledger;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse(std::env::args().skip(1))?;

    let start = Instant::now();
    let loaded = load(&args.ledger).map_err(|e| format!("error: {e}"))?;
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

    let state = Arc::new(AppState::new(ledger, parse_ms));
    serve(args, state)
}

#[tokio::main]
async fn serve(args: Args, state: Arc<AppState>) -> Result<(), String> {
    let listener =
        tokio::net::TcpListener::bind((args.host.as_str(), args.port))
            .await
            .map_err(|e| {
                format!(
                    "error: cannot listen on {}:{}: {e}",
                    args.host, args.port
                )
            })?;
    axum::serve(listener, router(state))
        .await
        .map_err(|e| format!("error: server failed: {e}"))
}
