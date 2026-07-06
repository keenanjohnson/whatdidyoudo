//! `wdyd` — thin bin: arg-parsing and wiring only. All logic lives in the library.

use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};

use whatdidyoudo::analyzers::{blast_radius, claims};
use whatdidyoudo::evidence::FsEvidence;
use whatdidyoudo::{AuditReport, ClaudeCodeAdapter, Discovery, SessionMeta, SourceAdapter};

/// Ask your coding agent "what did you do?" — and check its answers.
#[derive(Parser, Debug)]
#[command(name = "wdyd", version, about)]
struct Cli {
    /// Audit a specific transcript file instead of the latest session for this project.
    #[arg(long, value_name = "FILE")]
    session: Option<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Terminal)]
    format: Format,

    /// Exit non-zero (2) if any claim is CONTRADICTED — for pre-commit / CI gates.
    #[arg(long)]
    check: bool,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum Format {
    /// Colored tables for a terminal (default).
    #[default]
    Terminal,
    /// Pretty JSON for scripting.
    Json,
    /// GitHub-flavored Markdown, paste-ready into a PR.
    Md,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("wdyd: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();

    let session_path = match cli.session {
        Some(path) => path,
        None => resolve_latest()?,
    };

    let file = File::open(&session_path)
        .with_context(|| format!("opening transcript {}", session_path.display()))?;
    let events: Vec<_> = ClaudeCodeAdapter::parse(BufReader::new(file)).collect();

    let extracted = claims::extract(&events);
    let report = AuditReport {
        session: SessionMeta {
            path: session_path.display().to_string(),
            events: events.len(),
        },
        blast_radius: blast_radius::analyze(&events),
        claims: claims::verify(extracted, &events, &FsEvidence),
    };

    match cli.format {
        Format::Terminal => print!("{}", report.to_terminal()),
        Format::Json => println!("{}", report.to_json()),
        Format::Md => println!("{}", report.to_markdown()),
    }

    // --check turns a contradicted claim into a failing exit code.
    let contradicted = report.trust_summary().contradicted;
    Ok(if cli.check && contradicted > 0 {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    })
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
