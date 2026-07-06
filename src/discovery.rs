//! Discovery — resolve which session(s) to audit.
//!
//! Encodes the cwd the way Claude Code encodes it for `~/.claude/projects/<encoded-cwd>/`,
//! then selects the most recent *auditable* session. Path + metadata logic only; the one
//! content peek (noise filtering) is a cheap byte scan, never a full parse.
//! See `docs/architecture.md` §1.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Locates Claude Code session transcripts under a projects root.
pub struct Discovery {
    projects_root: PathBuf,
}

impl Discovery {
    /// Default root: `$HOME/.claude/projects`. `None` if `$HOME` is unset.
    pub fn from_home() -> Option<Self> {
        let home = std::env::var_os("HOME")?;
        Some(Self::with_root(
            Path::new(&home).join(".claude").join("projects"),
        ))
    }

    /// Explicit root — used by tests to stay off the real filesystem.
    pub fn with_root(projects_root: impl Into<PathBuf>) -> Self {
        Self {
            projects_root: projects_root.into(),
        }
    }

    /// Encode a cwd to its project directory name. Claude Code replaces every
    /// non-alphanumeric character with `-` (verified against real project dirs);
    /// the mapping is lossy, so we only ever encode forward, never decode.
    pub fn encode_cwd(cwd: &Path) -> String {
        cwd.to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect()
    }

    /// The `~/.claude/projects/<encoded>` directory for a cwd.
    pub fn project_dir(&self, cwd: &Path) -> PathBuf {
        self.projects_root.join(Self::encode_cwd(cwd))
    }

    /// Most recently modified auditable session for `cwd`, or `None` if the project
    /// has no sessions or only noise ones. Only top-level `*.jsonl` files are
    /// considered (subagent transcripts live in subdirectories).
    pub fn latest_session(&self, cwd: &Path) -> Option<PathBuf> {
        let dir = self.project_dir(cwd);
        let mut newest: Option<(SystemTime, PathBuf)> = None;
        for entry in fs::read_dir(&dir).ok()?.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|e| e != "jsonl") {
                continue;
            }
            let meta = entry.metadata().ok()?;
            if !meta.is_file() || is_noise_session(&path) {
                continue;
            }
            let mtime = meta.modified().ok()?;
            if newest.as_ref().is_none_or(|(t, _)| mtime > *t) {
                newest = Some((mtime, path));
            }
        }
        newest.map(|(_, p)| p)
    }
}

/// A session is auditable iff it contains at least one tool call; warmup and
/// `/clear`-only transcripts have none. Cheap substring scan over lines — no JSON
/// parsing — so discovery stays out of the ingestion layer's job.
fn is_noise_session(path: &Path) -> bool {
    let Ok(file) = fs::File::open(path) else {
        return true; // unreadable → not worth surfacing
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if line.contains("\"tool_use\"") {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn encodes_cwd_by_replacing_non_alphanumerics() {
        assert_eq!(
            Discovery::encode_cwd(Path::new("/Users/keen/Documents/GitHub/whatdidyoudo")),
            "-Users-keen-Documents-GitHub-whatdidyoudo"
        );
        // dots, underscores, and existing hyphens all collapse to '-'
        assert_eq!(
            Discovery::encode_cwd(Path::new("/a/b_c.d/e-f")),
            "-a-b-c-d-e-f"
        );
    }

    fn write(path: &Path, contents: &str, mtime: SystemTime) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        f.set_modified(mtime).unwrap();
    }

    const AUDITABLE: &str =
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash"}]}}"#;
    const NOISE: &str =
        r#"{"type":"user","message":{"content":"<command-name>/clear</command-name>"}}"#;

    #[test]
    fn picks_newest_auditable_session_and_skips_noise() {
        let root = std::env::temp_dir().join(format!("wdyd-disc-{}", std::process::id()));
        let cwd = Path::new("/proj/x");
        let dir = root.join(Discovery::encode_cwd(cwd));
        fs::create_dir_all(&dir).unwrap();

        let t = |secs| SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
        write(&dir.join("old.jsonl"), AUDITABLE, t(100));
        write(&dir.join("new.jsonl"), AUDITABLE, t(200));
        write(&dir.join("noise.jsonl"), NOISE, t(300)); // newest but not auditable

        let disc = Discovery::with_root(&root);
        assert_eq!(disc.latest_session(cwd), Some(dir.join("new.jsonl")));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn returns_none_when_only_noise() {
        let root = std::env::temp_dir().join(format!("wdyd-disc-noise-{}", std::process::id()));
        let cwd = Path::new("/proj/y");
        let dir = root.join(Discovery::encode_cwd(cwd));
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("a.jsonl"), NOISE, SystemTime::UNIX_EPOCH);

        let disc = Discovery::with_root(&root);
        assert_eq!(disc.latest_session(cwd), None);

        fs::remove_dir_all(&root).ok();
    }
}
