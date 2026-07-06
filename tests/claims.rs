//! Claims extraction + verification over a parsed fixture timeline.

use std::io::BufReader;
use std::path::Path;

use whatdidyoudo::analyzers::{claims, ClaimKind, Verdict};
use whatdidyoudo::evidence::Evidence;
use whatdidyoudo::{ClaudeCodeAdapter, Event, SourceAdapter};

fn events(name: &str) -> Vec<Event> {
    let path = format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    ClaudeCodeAdapter::parse(BufReader::new(std::fs::File::open(path).unwrap())).collect()
}

/// Filesystem stub: every file either exists or doesn't.
struct FileExists(bool);
impl Evidence for FileExists {
    fn file_exists(&self, _: &Path) -> bool {
        self.0
    }
}

#[test]
fn extracts_the_three_claims_from_the_final_message() {
    let claims = claims::extract(&events("session_typical.jsonl"));
    let kinds: Vec<_> = claims.iter().map(|c| c.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            ClaimKind::TestsPass,
            ClaimKind::BuildSucceeds,
            ClaimKind::FileCreated("src/hello.rs".into()),
        ]
    );
}

#[test]
fn verifies_tests_but_contradicts_the_failed_build() {
    let ev = events("session_typical.jsonl");
    let verdicts = claims::verify(claims::extract(&ev), &ev, &FileExists(false));

    for (claim, verdict) in &verdicts {
        match claim.kind {
            // `cargo test` succeeded in the transcript
            ClaimKind::TestsPass => assert!(matches!(verdict, Verdict::Verified(_))),
            // `cargo build --release` failed — the claim is a lie
            ClaimKind::BuildSucceeds => assert!(matches!(verdict, Verdict::Contradicted(_))),
            // file was written in-session but isn't on disk (FileExists(false))
            ClaimKind::FileCreated(_) => assert!(matches!(verdict, Verdict::Contradicted(_))),
            _ => {}
        }
    }
}

#[test]
fn file_creation_verifies_when_the_file_is_on_disk() {
    let ev = events("session_typical.jsonl");
    let verdicts = claims::verify(claims::extract(&ev), &ev, &FileExists(true));
    let (_, verdict) = verdicts
        .iter()
        .find(|(c, _)| matches!(c.kind, ClaimKind::FileCreated(_)))
        .unwrap();
    assert!(matches!(verdict, Verdict::Verified(_)));
}
