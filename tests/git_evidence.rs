//! Real git integration: LocalEvidence against a throwaway repo with controlled dates.
//! Exercises the actual `git` subprocess + parsing (the mock tests can't).

use std::fs;
use std::path::Path;
use std::process::Command;

use whatdidyoudo::evidence::{Evidence, LocalEvidence};
use whatdidyoudo::Timestamp;

/// Run git in `dir`, optionally stamping author+committer date, panicking on failure.
fn git(dir: &Path, date: Option<&str>, args: &[&str]) {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(dir).args(args);
    if let Some(d) = date {
        cmd.env("GIT_AUTHOR_DATE", d).env("GIT_COMMITTER_DATE", d);
    }
    let out = cmd.output().expect("run git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn write(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).unwrap();
}

#[test]
fn local_evidence_reads_commits_and_changed_files() {
    let dir = std::env::temp_dir().join(format!("wdyd-git-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();

    git(&dir, None, &["init", "-q"]);
    git(&dir, None, &["config", "user.email", "t@t.test"]);
    git(&dir, None, &["config", "user.name", "Test"]);
    git(&dir, None, &["config", "commit.gpgsign", "false"]);

    // Baseline commit, dated well before the "session".
    write(&dir, "base.txt", "one\n");
    git(&dir, None, &["add", "-A"]);
    git(
        &dir,
        Some("2020-01-01T00:00:00Z"),
        &["commit", "-q", "-m", "baseline"],
    );

    // In-session commit: modifies base.txt and adds feature.rs.
    write(&dir, "base.txt", "two\n");
    write(&dir, "feature.rs", "fn main() {}\n");
    git(&dir, None, &["add", "-A"]);
    git(
        &dir,
        Some("2026-06-01T00:00:00Z"),
        &["commit", "-q", "-m", "add feature"],
    );

    // An untracked file left in the working tree (an uncommitted "shell edit").
    write(&dir, "scratch.txt", "wip\n");

    let ev = LocalEvidence::new(&dir);
    let since = Timestamp::from_unix(1735689600).unwrap(); // 2025-01-01

    assert!(ev.is_git_repo());

    let commits = ev.commits_since(since);
    assert_eq!(commits.len(), 1, "only the 2026 commit is in-window");
    assert_eq!(commits[0].subject, "add feature");

    let changed: Vec<String> = ev
        .changed_files_since(since)
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert!(changed.contains(&"base.txt".to_string()), "{changed:?}");
    assert!(changed.contains(&"feature.rs".to_string()), "{changed:?}");
    assert!(
        changed.contains(&"scratch.txt".to_string()),
        "untracked file missing: {changed:?}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn non_repo_directory_is_not_a_git_repo() {
    let dir = std::env::temp_dir().join(format!("wdyd-nogit-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let ev = LocalEvidence::new(&dir);
    assert!(!ev.is_git_repo());
    assert!(ev
        .commits_since(Timestamp::from_unix(0).unwrap())
        .is_empty());
    fs::remove_dir_all(&dir).ok();
}
