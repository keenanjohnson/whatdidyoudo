//! Report rendering regressions — output-polish fixes caught by auditing this repo's
//! own build session: markdown rows broken by newlines, heredoc dumps in evidence
//! cells, dropped agent quotes, and hostile scope on broad tasks.

use whatdidyoudo::analyzers::blast_radius::{BlastRadius, FileTouch};
use whatdidyoudo::analyzers::claims::Claim;
use whatdidyoudo::analyzers::{ClaimKind, Verdict};
use whatdidyoudo::{AuditReport, SessionMeta, Timestamp};

fn claim(kind: ClaimKind, quote: &str) -> Claim {
    Claim {
        kind,
        ts: Timestamp::from_unix(1).unwrap(),
        quote: quote.to_string(),
    }
}

fn report(claims: Vec<(Claim, Verdict)>, blast_radius: BlastRadius) -> AuditReport {
    AuditReport {
        session: SessionMeta {
            path: "test.jsonl".into(),
            events: 1,
        },
        claims,
        blast_radius,
    }
}

/// Every markdown table row must stay on one line — a raw newline in an evidence
/// cell terminates a GFM row and breaks the table when pasted into a PR.
#[test]
fn markdown_rows_survive_multiline_evidence() {
    let evidence =
        "`git commit -m \"$(cat <<'EOF'\nAdd the thing\n\ntests pass\nEOF\n)\"` exited ok";
    let r = report(
        vec![(
            claim(ClaimKind::Committed, "Committed the fix."),
            Verdict::Verified(evidence.into()),
        )],
        BlastRadius::default(),
    );
    let md = r.to_markdown();
    let row = md.lines().find(|l| l.contains("committed")).unwrap();
    assert!(row.starts_with('|') && row.ends_with('|'));
    // The heredoc body must not appear as stray lines outside the row.
    assert!(!md.contains("\nAdd the thing"));
}

/// Evidence cells show the first line, capped at ~80 chars, in both renderers.
/// JSON keeps the full string — it's the machine-readable format.
#[test]
fn evidence_is_truncated_in_tables_but_not_json() {
    let long = format!("`cargo test --workspace {}` exited ok", "x".repeat(100));
    let r = report(
        vec![(
            claim(ClaimKind::TestsPass, "tests pass"),
            Verdict::Verified(long.clone()),
        )],
        BlastRadius::default(),
    );

    let ellipsized = |s: &str| s.contains('…') && !s.contains(&long);
    assert!(ellipsized(&r.to_terminal()), "terminal must truncate");
    assert!(ellipsized(&r.to_markdown()), "markdown must truncate");

    let json: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(json["claims"][0]["evidence"], long.as_str());
}

/// A contradiction is an accusation — it must show what the agent actually said.
#[test]
fn contradicted_claims_show_the_agents_words() {
    let r = report(
        vec![
            (
                claim(ClaimKind::BuildSucceeds, "the release build succeeds."),
                Verdict::Contradicted("`cargo build --release` failed".into()),
            ),
            (
                claim(ClaimKind::TestsPass, "the tests pass."),
                Verdict::Verified("`cargo test` exited ok".into()),
            ),
        ],
        BlastRadius::default(),
    );

    let quoted = "agent said: \"the release build succeeds.\"";
    assert!(r.to_terminal().contains(quoted));
    assert!(r.to_markdown().contains(quoted));
    // Verified claims don't need the quote in the tables…
    assert!(!r.to_terminal().contains("agent said: \"the tests pass.\""));
    // …but JSON carries it for every claim.
    let json: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(json["claims"][0]["quote"], "the release build succeeds.");
    assert_eq!(json["claims"][1]["quote"], "the tests pass.");
}

/// When the user gave a broad task (no files named), scope has no signal: no red
/// "OUT OF SCOPE" per file, "n/a" in the summary, nulls in JSON.
#[test]
fn broad_task_scope_is_presented_neutrally() {
    let br = BlastRadius {
        files: vec![FileTouch {
            path: "src/lib.rs".into(),
            tool: "Write".into(),
            in_scope: false,
        }],
        broad_task: true,
    };
    let r = report(vec![], br);

    let term = r.to_terminal();
    assert!(term.contains("scope n/a (broad task)"));
    assert!(!term.contains("OUT OF SCOPE"));
    assert!(!term.contains("scope 0%"));

    let md = r.to_markdown();
    assert!(md.contains("scope n/a (broad task)"));
    assert!(!md.contains("out of scope"));

    let json: serde_json::Value = serde_json::from_str(&r.to_json()).unwrap();
    assert_eq!(json["trust"]["scope_pct"], serde_json::Value::Null);
    assert_eq!(json["blast_radius"][0]["in_scope"], serde_json::Value::Null);
}

/// A task that names files keeps the real percentage and the per-file verdicts.
#[test]
fn named_files_keep_the_scope_percentage() {
    let br = BlastRadius {
        files: vec![
            FileTouch {
                path: "src/lib.rs".into(),
                tool: "Write".into(),
                in_scope: true,
            },
            FileTouch {
                path: "scripts/deploy.sh".into(),
                tool: "Edit".into(),
                in_scope: false,
            },
        ],
        broad_task: false,
    };
    let r = report(vec![], br);
    assert!(r.to_terminal().contains("scope 50%"));
    assert!(r.to_terminal().contains("OUT OF SCOPE"));
}
