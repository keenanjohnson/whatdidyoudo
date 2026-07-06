//! Fixture regression net: each real transcript in `fixtures/` parses to a stable
//! `Event` timeline. Format churn shows up here as a failing snapshot, not a user bug.
//!
//! NOTE: fixtures are currently raw (un-anonymized) and gitignored; both these and the
//! generated snapshots must be anonymized before they are committed. See docs/roadmap.md.

use std::fs::File;
use std::io::BufReader;

use whatdidyoudo::{ClaudeCodeAdapter, SourceAdapter};

fn events_for(name: &str) -> Vec<whatdidyoudo::Event> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let reader = BufReader::new(File::open(&path).expect("fixture present"));
    ClaudeCodeAdapter::parse(reader).collect()
}

#[test]
fn rich_session_timeline() {
    let events = events_for("rich_bash_edit_session.jsonl");
    insta::assert_debug_snapshot!(events);
}

#[test]
fn noise_session_timeline() {
    let events = events_for("noise_local_command.jsonl");
    insta::assert_debug_snapshot!(events);
}
