//! `wdyd` — thin bin: arg-parsing and wiring only. All logic lives in the library.

use clap::Parser;

/// Ask your coding agent "what did you do?" — and check its answers.
#[derive(Parser, Debug)]
#[command(name = "wdyd", version, about)]
struct Cli {
    /// Audit a specific session id instead of the latest for this project.
    #[arg(long)]
    session: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let _cli = Cli::parse();
    // Pipeline wiring lands in M1: discovery → ingestion → analyzers → report.
    println!("wdyd: scaffold only — analysis pipeline arrives in M1.");
    Ok(())
}
