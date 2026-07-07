//! Claims extractor + verifier. Two halves that never learn each other's job:
//! the extractor pattern-matches `AssistantText` into claims and knows nothing about
//! truth; the verifier matches claims against evidence and knows nothing about regexes.
//! See `docs/architecture.md` §3.
//!
//! v1 verifies `TestsPass`, `BuildSucceeds`, `FileCreated`, and `Committed` (git-backed);
//! `BugFixed` has no mechanical check.

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
        if let Some(path) = file_created_re()
            .captures_iter(text)
            .map(|caps| caps.get(1).unwrap().as_str())
            .find(|p| !is_abbreviation(p))
        {
            claims.push(Claim {
                kind: ClaimKind::FileCreated(path.into()),
                ts: *ts,
                quote: quote.to_string(),
            });
        }
        if claims_committed(text) {
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

/// Prose abbreviations that the filename capture mistakes for paths ("i.e", "e.g" —
/// the final period is never captured, since the pattern requires a character after it).
fn is_abbreviation(candidate: &str) -> bool {
    matches!(candidate.to_ascii_lowercase().as_str(), "i.e" | "e.g")
}

/// A "committed" mention with an optional preceding negation ("not committed",
/// "haven't committed yet"). Negated mentions are status updates, not claims.
fn committed_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)\b(?:(?P<neg>not|never|without|nothing|(?:have|has|is|was|are|were|wo|do|does|did)n[’']?t)\s+(?:yet\s+|been\s+|being\s+)*)?committed\b",
        )
        .unwrap()
    })
}

/// True when the text mentions "committed" at least once *without* a negation in front.
fn claims_committed(text: &str) -> bool {
    committed_re()
        .captures_iter(text)
        .any(|c| c.name("neg").is_none())
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
/// confirms it. Finding none is only `Unverified`, never `Contradicted` — the agent may
/// be recapping a commit from an earlier session, and `git log --since` windowing can
/// miss rebases/amends. Absence of evidence isn't evidence of a lie.
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
        None => Verdict::Unverified("no commit found in the session window".into()),
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
    let run = runs.iter().rfind(|r| r.kind == kind && r.ts <= claim_ts);
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
    // The disk check must use the writes' own paths, not the claim's: the agent claims a
    // bare name ("created discovery.rs") while the tool wrote the real, often absolute,
    // path — checking the bare name against the root would contradict a true claim.
    // Several writes can suffix-match one claimed name; any of them on disk verifies.
    let written: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::ToolCall {
                tool, input, ts, ..
            } if *ts <= claim_ts => write_path_matching(tool, input, &target),
            _ => None,
        })
        .collect();
    if written.is_empty() {
        return if evidence.file_exists(path) {
            Verdict::Unverified("exists on disk, but no write to it this session".into())
        } else {
            Verdict::Unverified("no write to this file in the session".into())
        };
    }
    if written.iter().any(|p| evidence.file_exists(Path::new(p))) {
        Verdict::Verified("written this session, present on disk".into())
    } else {
        Verdict::Contradicted("claimed created, but not found on disk".into())
    }
}

/// The path this tool call writes, if it matches `target` — on equality or suffix so a
/// relative claim ("main_window.py") still pairs with an absolute write path.
fn write_path_matching<'a>(
    tool: &str,
    input: &'a crate::ingestion::ToolInput,
    target: &str,
) -> Option<&'a str> {
    let field = match tool {
        "Write" | "Edit" | "MultiEdit" => "file_path",
        "NotebookEdit" => "notebook_path",
        _ => return None,
    };
    input
        .0
        .get(field)
        .map(String::as_str)
        .filter(|p| *p == target || p.ends_with(target) || target.ends_with(p))
}
