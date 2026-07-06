//! `wdyd` — thin bin: arg-parsing and wiring only. All logic lives in the library.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use whatdidyoudo::analyzers::blast_radius;
use whatdidyoudo::{AuditReport, ClaudeCodeAdapter, Discovery, SessionMeta, SourceAdapter};

/// Ask your coding agent "what did you do?" — and check its answers.
#[derive(Parser, Debug)]
#[command(name = "wdyd", version, about)]
struct Cli {
    /// Audit a specific transcript file instead of the latest session for this project.
    #[arg(long, value_name = "FILE")]
    session: Option<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let session_path = match cli.session {
        Some(path) => path,
        None => resolve_latest()?,
    };

    let file = File::open(&session_path)
        .with_context(|| format!("opening transcript {}", session_path.display()))?;
    let events: Vec<_> = ClaudeCodeAdapter::parse(BufReader::new(file)).collect();

    let report = AuditReport {
        session: SessionMeta {
            path: session_path.display().to_string(),
            events: events.len(),
        },
        blast_radius: blast_radius::analyze(&events),
    };

    print!("{}", report.to_terminal());
    Ok(())
}

/// Find the most recent auditable session for the current directory.
fn resolve_latest() -> Result<PathBuf> {
    let cwd = std::env::current_dir().context("reading current directory")?;
    let discovery = Discovery::from_home().context("$HOME is not set")?;
    discovery.latest_session(&cwd).with_context(|| {
        format!(
            "no auditable Claude Code session found for {}. Pass --session <file> to audit a specific transcript.",
            cwd.display()
        )
    })
}
