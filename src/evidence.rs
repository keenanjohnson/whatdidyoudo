//! Evidence provider — the verifier's only window onto reality outside the transcript.
//!
//! Git via `std::process::Command` + filesystem checks. Trait-based for mocking in tests.
//! This trait plus transcript reading is the tool's ENTIRE I/O surface. No network, ever.
//! See `docs/architecture.md` §4.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ingestion::Timestamp;

/// One commit, for verifying `Committed` claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Commit {
    pub hash: String,
    pub subject: String,
    pub ts: Timestamp,
}

pub trait Evidence {
    fn file_exists(&self, p: &Path) -> bool;

    /// Whether the working directory is inside a git repo. Distinguishes "can't check"
    /// from "checked and found nothing" — a `Committed` claim in a non-repo is
    /// Unverified, but in a repo with no matching commit it's Contradicted.
    fn is_git_repo(&self) -> bool {
        false
    }

    /// Commits dated at/after `since`, most recent first. Empty when not a repo.
    fn commits_since(&self, since: Timestamp) -> Vec<Commit> {
        let _ = since;
        Vec::new()
    }

    /// Files changed (committed or in the working tree) since `since`. This is how the
    /// blast radius catches shell/`sed` edits that never went through Write/Edit.
    fn changed_files_since(&self, since: Timestamp) -> Vec<PathBuf> {
        let _ = since;
        Vec::new()
    }
}

/// Real evidence: filesystem + git, rooted at a directory (normally the process cwd, so
/// `wdyd` run inside the audited project sees its files and history).
///
/// Caveat: auditing a session from a *different* project via `--session` checks git
/// against this root, not the session's — git evidence is only meaningful when the root
/// is the project the session belongs to.
pub struct LocalEvidence {
    root: PathBuf,
}

impl LocalEvidence {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Rooted at the current working directory.
    pub fn cwd() -> Self {
        Self::new(std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    /// Run `git -C <root>` with args, returning stdout on success. `None` on any failure
    /// (git missing, not a repo, nonzero exit) so evidence degrades to empty, never panics.
    fn git(&self, args: &[&str]) -> Option<String> {
        let out = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(args)
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

impl Evidence for LocalEvidence {
    fn file_exists(&self, p: &Path) -> bool {
        // `join` returns `p` unchanged if it's absolute, else resolves against the root.
        self.root.join(p).exists()
    }

    fn is_git_repo(&self) -> bool {
        self.git(&["rev-parse", "--is-inside-work-tree"])
            .is_some_and(|s| s.trim() == "true")
    }

    fn commits_since(&self, since: Timestamp) -> Vec<Commit> {
        let arg = format!("--since={}", since.to_rfc3339());
        // %H hash, %ct committer-date unix, %s subject — unit-separated so subjects
        // containing anything can't break parsing.
        let Some(out) = self.git(&["log", &arg, "--format=%H%x1f%ct%x1f%s"]) else {
            return Vec::new();
        };
        out.lines().filter_map(parse_commit).collect()
    }

    fn changed_files_since(&self, since: Timestamp) -> Vec<PathBuf> {
        // Baseline: the last commit strictly before the session began. Without one
        // (session predates the repo), we can't scope a diff — return nothing.
        let before = format!("--before={}", since.to_rfc3339());
        let base = self
            .git(&["rev-list", "-1", &before, "HEAD"])
            .unwrap_or_default();
        let base = base.trim();
        if base.is_empty() {
            return Vec::new();
        }

        let mut files = BTreeSet::new();
        if let Some(out) = self.git(&["diff", "--name-only", base]) {
            files.extend(out.lines().filter(|l| !l.is_empty()).map(PathBuf::from));
        }
        // New untracked files aren't in `git diff` — add them explicitly.
        if let Some(out) = self.git(&["ls-files", "--others", "--exclude-standard"]) {
            files.extend(out.lines().filter(|l| !l.is_empty()).map(PathBuf::from));
        }
        files.into_iter().collect()
    }
}

fn parse_commit(line: &str) -> Option<Commit> {
    let mut parts = line.splitn(3, '\u{1f}');
    let hash = parts.next()?.to_string();
    let ts = Timestamp::from_unix(parts.next()?.parse().ok()?)?;
    let subject = parts.next().unwrap_or("").to_string();
    Some(Commit { hash, subject, ts })
}
