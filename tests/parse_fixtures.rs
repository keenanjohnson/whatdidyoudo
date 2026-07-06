//! Fixture regression net: each transcript in `fixtures/` parses to a stable `Event`
//! timeline. Format churn shows up here as a failing snapshot, not a user bug report.
//!
//! Fixtures are hand-written synthetic JSONL that mirror the real Claude Code schema
//! (see `docs/architecture.md` §2) — no real transcript data is committed.

use std::fs::File;
use std::io::BufReader;

use whatdidyoudo::{ClaudeCodeAdapter, Event, SourceAdapter};

fn events_for(name: &str) -> Vec<Event> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let reader = BufReader::new(File::open(&path).expect("fixture present"));
    ClaudeCodeAdapter::parse(reader).collect()
}

#[test]
fn typical_session_timeline() {
    // Prompt, dropped thinking, text, Write + Bash calls with Ok/Failed outcomes,
    // a noise line as Unknown, and a final claim.
    insta::assert_debug_snapshot!(events_for("session_typical.jsonl"));
}

#[test]
fn noise_session_timeline() {
    // No tool calls — only meta/command messages. Discovery will later classify this
    // as a noise session; ingestion still emits it faithfully.
    insta::assert_debug_snapshot!(events_for("session_noise.jsonl"));
}
