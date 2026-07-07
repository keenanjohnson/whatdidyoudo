//! Claims extraction + verification over a parsed fixture timeline.

use std::io::BufReader;
use std::path::Path;

use whatdidyoudo::analyzers::{claims, ClaimKind, Verdict};
use whatdidyoudo::evidence::{Commit, Evidence};
use whatdidyoudo::{ClaudeCodeAdapter, Event, SourceAdapter, Timestamp};

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

/// Git stub: configurable repo-ness and commit list.
struct GitStub {
    repo: bool,
    commits: Vec<Commit>,
}
impl Evidence for GitStub {
    fn file_exists(&self, _: &Path) -> bool {
        false
    }
    fn is_git_repo(&self) -> bool {
        self.repo
    }
    fn commits_since(&self, _: Timestamp) -> Vec<Commit> {
        self.commits.clone()
    }
}

/// Path-aware stub: a repo with a fixed set of files on disk and no commits.
struct RepoWithFiles(Vec<&'static str>);
impl Evidence for RepoWithFiles {
    fn file_exists(&self, p: &Path) -> bool {
        self.0.contains(&p.to_str().unwrap_or_default())
    }
    fn is_git_repo(&self) -> bool {
        true
    }
}

fn verdict_for(verdicts: &[(claims::Claim, Verdict)], want: &ClaimKind) -> Verdict {
    verdicts
        .iter()
        .find(|(c, _)| &c.kind == want)
        .map(|(_, v)| v.clone())
        .expect("claim present")
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

// Regression tests for the three false verdicts caught by auditing this repo's own
// build session (see fixtures/session_false_verdicts.jsonl).

#[test]
fn negated_committed_mention_is_not_a_claim() {
    // "not committed yet" in the recap message must not extract; only the final
    // message's positive "committed" does.
    let claims = claims::extract(&events("session_false_verdicts.jsonl"));
    let kinds: Vec<_> = claims.iter().map(|c| c.kind.clone()).collect();
    assert_eq!(
        kinds,
        vec![
            ClaimKind::TestsPass,
            ClaimKind::FileCreated("discovery.rs".into()),
            ClaimKind::Committed,
        ]
    );
}

#[test]
fn relative_claim_verifies_against_the_absolute_write_path() {
    // The agent claimed "created discovery.rs"; the Write used /repo/src/discovery.rs.
    // The disk check must follow the write's path, not the bare claimed name.
    let ev = events("session_false_verdicts.jsonl");
    let disk = RepoWithFiles(vec!["/repo/src/discovery.rs"]);
    let v = claims::verify(claims::extract(&ev), &ev, &disk);
    assert!(matches!(
        verdict_for(&v, &ClaimKind::FileCreated("discovery.rs".into())),
        Verdict::Verified(_)
    ));
}

#[test]
fn commit_command_mentioning_tests_is_not_test_evidence() {
    // The only Bash run is a `git commit` whose heredoc message says "tests pass" —
    // that must not verify a TestsPass claim.
    let ev = events("session_false_verdicts.jsonl");
    let disk = RepoWithFiles(vec!["/repo/src/discovery.rs"]);
    let v = claims::verify(claims::extract(&ev), &ev, &disk);
    assert!(matches!(
        verdict_for(&v, &ClaimKind::TestsPass),
        Verdict::Unverified(_)
    ));
}

#[test]
fn extracts_a_committed_claim() {
    let claims = claims::extract(&events("session_committed.jsonl"));
    assert_eq!(
        claims.iter().map(|c| &c.kind).collect::<Vec<_>>(),
        vec![&ClaimKind::Committed]
    );
}

#[test]
fn committed_verdict_follows_git_state() {
    let ev = events("session_committed.jsonl");
    let commit = Commit {
        hash: "a1b2c3d4ef".into(),
        subject: "wire up the parser".into(),
        ts: Timestamp::from_unix(1000).unwrap(), // before the claim
    };

    // repo + a commit in-window → Verified
    let repo_with_commit = GitStub {
        repo: true,
        commits: vec![commit],
    };
    let v = claims::verify(claims::extract(&ev), &ev, &repo_with_commit);
    assert!(matches!(
        verdict_for(&v, &ClaimKind::Committed),
        Verdict::Verified(_)
    ));

    // repo, no commits → Unverified, not Contradicted: the claim may recap an earlier
    // session's commit, and log windowing can miss rebases/amends
    let repo_no_commits = GitStub {
        repo: true,
        commits: vec![],
    };
    let v = claims::verify(claims::extract(&ev), &ev, &repo_no_commits);
    assert!(matches!(
        verdict_for(&v, &ClaimKind::Committed),
        Verdict::Unverified(_)
    ));

    // not a repo → can't say
    let no_repo = GitStub {
        repo: false,
        commits: vec![],
    };
    let v = claims::verify(claims::extract(&ev), &ev, &no_repo);
    assert!(matches!(
        verdict_for(&v, &ClaimKind::Committed),
        Verdict::Unverified(_)
    ));
}
