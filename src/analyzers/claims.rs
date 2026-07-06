//! Claims extractor + verifier. Two halves that never learn each other's job:
//! the extractor pattern-matches `AssistantText` into claims and knows nothing about
//! truth; the verifier matches claims against evidence and knows nothing about regexes.
//! See `docs/architecture.md` §3.
//!
//! v1 verifies `TestsPass`, `BuildSucceeds`, and `FileCreated`. `Committed` awaits the
//! git evidence provider; `BugFixed` has no mechanical check.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::analyzers::commands::{self, CommandKind, CommandRun};
use crate::analyzers::{ClaimKind, Verdict};
use crate::evidence::Evidence;
use crate::ingestion::{Event, Timestamp, ToolOutcome};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub kind: ClaimKind,
    pub ts: Timestamp,
    /// The agent's own words that triggered the claim — shown in the report.
    pub quote: String,
}

// ============================ Extractor ============================
// Pattern-matches assistant prose into claims. Knows nothing about truth.

/// Extract claims from every assistant message. At most one claim of each kind per
/// message, to avoid double-counting a repeated phrase.
pub fn extract(events: &[Event]) -> Vec<Claim> {
    let mut claims = Vec::new();
    for event in events {
        let Event::AssistantText { ts, text } = event else {
            continue;
        };
        let quote = text.trim();
        if tests_pass_re().is_match(text) {
            claims.push(Claim {
                kind: ClaimKind::TestsPass,
                ts: *ts,
                quote: quote.to_string(),
            });
        }
        if build_ok_re().is_match(text) {
            claims.push(Claim {
                kind: ClaimKind::BuildSucceeds,
                ts: *ts,
                quote: quote.to_string(),
            });
        }
        if let Some(caps) = file_created_re().captures(text) {
            let path = caps.get(1).unwrap().as_str();
            claims.push(Claim {
                kind: ClaimKind::FileCreated(path.into()),
                ts: *ts,
                quote: quote.to_string(),
            });
        }
        if committed_re().is_match(text) {
            claims.push(Claim {
                kind: ClaimKind::Committed,
                ts: *ts,
                quote: quote.to_string(),
            });
        }
    }
    claims
}

fn tests_pass_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\btests?\b[^.\n]{0,20}\bpass(?:es|ing|ed)?\b").unwrap())
}

fn build_ok_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:builds?|compil\w+)\b[^.\n]{0,20}\b(?:succeed\w*|success\w*|pass\w*|clean|green|works?|fine|ok)\b",
        )
        .unwrap()
    })
}

fn file_created_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\b(?:created|added|wrote|write|generated|new file)\b[^.\n]{0,40}?([\w./\-]+\.[A-Za-z0-9]+)").unwrap()
    })
}

fn committed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bcommitted\b").unwrap())
}

// ============================ Verifier ============================
// Interrogates evidence dated before each claim. Knows nothing about regexes.

pub fn verify(
    claims: Vec<Claim>,
    events: &[Event],
    evidence: &dyn Evidence,
) -> Vec<(Claim, Verdict)> {
    let runs = commands::analyze(events);
    let session_start = crate::ingestion::session_start(events);
    claims
        .into_iter()
        .map(|claim| {
            let verdict = match &claim.kind {
                ClaimKind::TestsPass => command_verdict(&runs, CommandKind::Test, claim.ts, "test"),
                ClaimKind::BuildSucceeds => {
                    command_verdict(&runs, CommandKind::Build, claim.ts, "build")
                }
                ClaimKind::FileCreated(path) => file_verdict(path, events, claim.ts, evidence),
                ClaimKind::Committed => committed_verdict(session_start, claim.ts, evidence),
                ClaimKind::BugFixed => {
                    Verdict::Unverified("no mechanical check for bug fixes in v1".into())
                }
            };
            (claim, verdict)
        })
        .collect()
}

/// Verdict for a `Committed` claim: a commit in the session window (up to the claim)
/// confirms it; a repo with none contradicts it; no repo can't say.
fn committed_verdict(
    session_start: Option<Timestamp>,
    claim_ts: Timestamp,
    evidence: &dyn Evidence,
) -> Verdict {
    if !evidence.is_git_repo() {
        return Verdict::Unverified("not a git repository".into());
    }
    let Some(start) = session_start else {
        return Verdict::Unverified("no session timeline to bound commits".into());
    };
    match evidence
        .commits_since(start)
        .into_iter()
        .find(|c| c.ts <= claim_ts)
    {
        Some(c) => {
            let short = &c.hash[..c.hash.len().min(8)];
            Verdict::Verified(format!("commit {short} \"{}\"", c.subject))
        }
        None => Verdict::Contradicted("no commit found in the session window".into()),
    }
}

/// Verdict for a `TestsPass` / `BuildSucceeds` claim: the latest matching command that
/// completed at or before the claim decides it.
fn command_verdict(
    runs: &[CommandRun],
    kind: CommandKind,
    claim_ts: Timestamp,
    label: &str,
) -> Verdict {
    let run = runs
        .iter()
        .filter(|r| r.kind == kind && r.ts <= claim_ts)
        .next_back();
    match run {
        None => Verdict::Unverified(format!("no {label} command ran before the claim")),
        Some(r) => match r.outcome {
            ToolOutcome::Ok => Verdict::Verified(format!("`{}` exited ok", r.command)),
            ToolOutcome::Failed => Verdict::Contradicted(format!("`{}` failed", r.command)),
            ToolOutcome::Interrupted => {
                Verdict::Unverified(format!("`{}` was interrupted", r.command))
            }
            ToolOutcome::Unknown => Verdict::Unverified(format!("`{}` outcome unknown", r.command)),
        },
    }
}

fn file_verdict(
    path: &Path,
    events: &[Event],
    claim_ts: Timestamp,
    evidence: &dyn Evidence,
) -> Verdict {
    let target = path.to_string_lossy();
    let written = events.iter().any(|e| match e {
        Event::ToolCall {
            tool, input, ts, ..
        } => *ts <= claim_ts && is_write_of(tool, input, &target),
        _ => false,
    });
    match (written, evidence.file_exists(path)) {
        (true, true) => Verdict::Verified("written this session, present on disk".into()),
        (true, false) => Verdict::Contradicted("claimed created, but not found on disk".into()),
        (false, true) => {
            Verdict::Unverified("exists on disk, but no write to it this session".into())
        }
        (false, false) => Verdict::Unverified("no write to this file in the session".into()),
    }
}

/// Does this tool call write to `target`? Matches on path equality or suffix so a
/// relative claim ("main_window.py") still pairs with an absolute write path.
fn is_write_of(tool: &str, input: &crate::ingestion::ToolInput, target: &str) -> bool {
    let field = match tool {
        "Write" | "Edit" | "MultiEdit" => "file_path",
        "NotebookEdit" => "notebook_path",
        _ => return false,
    };
    input
        .0
        .get(field)
        .is_some_and(|p| p == target || p.ends_with(target) || target.ends_with(p.as_str()))
}
