//! Blast-radius analyzer — which files the agent actually touched, and whether each
//! was in scope for the task. Pure over `&[Event]`; no I/O. See `docs/architecture.md` §3.
//!
//! Scope heuristic (v1): a touched path is in-scope if it — or its file name — is
//! mentioned in the user's messages. Git cross-checking is a later M1 step.

use std::path::Path;
use std::sync::OnceLock;

use regex::Regex;

use crate::ingestion::Event;

/// Tool calls that write to the filesystem, and the input field naming the target path.
const WRITE_TOOLS: &[(&str, &str)] = &[
    ("Write", "file_path"),
    ("Edit", "file_path"),
    ("MultiEdit", "file_path"),
    ("NotebookEdit", "notebook_path"),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTouch {
    pub path: String,
    /// The tool that first touched it (Write/Edit/…).
    pub tool: String,
    pub in_scope: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlastRadius {
    pub files: Vec<FileTouch>,
    /// True when the user's messages named no concrete file or path — the task was
    /// broad ("build the tool"), so per-file scope has no signal and renderers must
    /// present it neutrally instead of accusing every touch of being out of scope.
    pub broad_task: bool,
}

impl BlastRadius {
    pub fn out_of_scope_count(&self) -> usize {
        self.files.iter().filter(|f| !f.in_scope).count()
    }
}

pub fn analyze(events: &[Event]) -> BlastRadius {
    let user_text = collect_user_text(events);
    let mut files: Vec<FileTouch> = Vec::new();

    for event in events {
        let Event::ToolCall { tool, input, .. } = event else {
            continue;
        };
        let Some(path) = write_target(tool, input) else {
            continue;
        };
        // First touch wins; later edits to the same file don't add rows.
        if files.iter().any(|f| f.path == path) {
            continue;
        }
        files.push(FileTouch {
            in_scope: mentioned(&user_text, &path),
            tool: tool.clone(),
            path,
        });
    }

    BlastRadius {
        broad_task: !names_paths(&user_text),
        files,
    }
}

/// Did the user name any file or path at all? A slashed path ("src/hello.rs") or a
/// dotted file name ("hello.rs"), skipping prose abbreviations ("i.e.", "e.g.").
/// The extension must start with a letter so version numbers ("1.2.3") don't count.
fn names_paths(user_text: &str) -> bool {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"[\w.-]+/[\w./-]+|\b[\w-]+\.[A-Za-z]\w*\b").unwrap());
    re.find_iter(user_text)
        .any(|m| !matches!(m.as_str().to_ascii_lowercase().as_str(), "i.e" | "e.g"))
}

/// Fold git-derived changes into an existing blast radius: any file changed in the
/// session's git window that wasn't already captured as a Write/Edit is added and
/// tagged `shell/git` — these are the edits made via Bash/`sed` (or committed directly).
pub fn merge_git_changes(br: &mut BlastRadius, events: &[Event], changed: &[std::path::PathBuf]) {
    let user_text = collect_user_text(events);
    for path in changed {
        let path = path.to_string_lossy().to_string();
        if br.files.iter().any(|f| same_file(&f.path, &path)) {
            continue;
        }
        br.files.push(FileTouch {
            in_scope: mentioned(&user_text, &path),
            tool: "shell/git".to_string(),
            path,
        });
    }
}

/// Loose path match for dedup: equal, or one is a suffix of the other (git paths are
/// repo-relative; tool paths may be absolute).
fn same_file(a: &str, b: &str) -> bool {
    a == b || a.ends_with(b) || b.ends_with(a)
}

fn write_target(tool: &str, input: &crate::ingestion::ToolInput) -> Option<String> {
    let field = WRITE_TOOLS.iter().find(|(t, _)| *t == tool)?.1;
    input.0.get(field).cloned()
}

fn collect_user_text(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|e| match e {
            Event::UserMessage { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// In scope if the full path or just the file name appears in the user's messages.
fn mentioned(user_text: &str, path: &str) -> bool {
    if user_text.contains(path) {
        return true;
    }
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| user_text.contains(name))
}
