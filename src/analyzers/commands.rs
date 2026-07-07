//! Commands analyzer — join each Bash `ToolCall` with its `ToolResult` outcome and
//! classify it (test / build / other). Pure over `&[Event]`. See `docs/architecture.md` §3.
//!
//! The verifier consumes these runs to check `TestsPass` / `BuildSucceeds` claims.

use std::collections::HashMap;

use crate::ingestion::{Event, Timestamp, ToolOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    Test,
    Build,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRun {
    /// When the command *completed* (the result's timestamp) — what claim ordering uses.
    pub ts: Timestamp,
    pub command: String,
    pub kind: CommandKind,
    pub outcome: ToolOutcome,
}

/// Rough command classification. Deliberately small and mechanical, but structural:
/// only the program word of each shell segment (plus a launcher's sub-command, e.g.
/// `cargo test`) can classify. Prose in heredoc bodies, `echo` strings, or commit
/// messages never counts — a `git commit -m "tests pass"` is not a test run.
pub fn classify(command: &str) -> CommandKind {
    let mut kind = CommandKind::Other;
    for segment in segments(command) {
        match classify_segment(segment) {
            CommandKind::Test => return CommandKind::Test,
            CommandKind::Build => kind = CommandKind::Build,
            CommandKind::Other => {}
        }
    }
    kind
}

const TEST_RUNNERS: &[&str] = &[
    "pytest",
    "jest",
    "vitest",
    "tox",
    "rspec",
    "ctest",
    "gotestsum",
    "cypress",
    "playwright",
];
/// Programs whose first sub-command word decides the kind (`cargo test`, `npm run build`).
const LAUNCHERS: &[&str] = &[
    "cargo", "npm", "pnpm", "yarn", "bun", "go", "make", "just", "rake", "mix", "dotnet", "gradle",
    "gradlew", "mvn", "python", "python3", "node", "deno", "uv", "poetry",
];
const BUILD_PROGS: &[&str] = &[
    "make",
    "cmake",
    "cc",
    "gcc",
    "g++",
    "clang",
    "clang++",
    "tsc",
    "rustc",
    "javac",
    "xcodebuild",
];

fn is_test_word(word: &str) -> bool {
    word.contains("test") || TEST_RUNNERS.contains(&word)
}

fn classify_segment(segment: &str) -> CommandKind {
    let mut tokens = segment
        .split_whitespace()
        .skip_while(|t| is_env_assignment(t));
    let Some(first) = tokens.next() else {
        return CommandKind::Other;
    };
    let prog = first.rsplit('/').next().unwrap_or(first).to_lowercase();

    if is_test_word(&prog) {
        return CommandKind::Test;
    }
    if LAUNCHERS.contains(&prog.as_str()) {
        let mut args = tokens.filter(|t| !t.starts_with('-'));
        let mut sub = args.next();
        // `npm run test:ci`, `cargo x build` — the word after the indirection decides.
        if matches!(sub, Some("run" | "exec" | "x")) {
            sub = args.next();
        }
        if let Some(sub) = sub.map(str::to_lowercase) {
            if is_test_word(&sub) {
                return CommandKind::Test;
            }
            if sub.contains("build") || sub.contains("compile") {
                return CommandKind::Build;
            }
        }
    }
    if BUILD_PROGS.contains(&prog.as_str()) || prog.contains("build") || prog.contains("compile") {
        return CommandKind::Build;
    }
    CommandKind::Other
}

/// Split a command into shell segments: heredoc bodies dropped, then lines split on
/// `|`, `;`, `&` (which also covers `&&` / `||` via empty segments).
fn segments(command: &str) -> impl Iterator<Item = &str> {
    let mut terminator: Option<&str> = None;
    command
        .lines()
        .filter(move |line| {
            if let Some(t) = terminator {
                if line.trim() == t {
                    terminator = None;
                }
                return false;
            }
            terminator = heredoc_terminator(line);
            true
        })
        .flat_map(|line| line.split(['|', ';', '&']))
}

/// The delimiter word of a heredoc started on this line (`<<EOF`, `<<-'EOF'`), if any.
fn heredoc_terminator(line: &str) -> Option<&str> {
    let rest = line[line.find("<<")? + 2..]
        .trim_start_matches('-')
        .trim_start();
    let word = rest
        .split(|c: char| c.is_whitespace() || c == ')')
        .next()
        .unwrap_or("")
        .trim_matches(['\'', '"']);
    (!word.is_empty()).then_some(word)
}

/// `FOO=bar` prefixes before the program word (`RUST_BACKTRACE=1 cargo test`).
fn is_env_assignment(token: &str) -> bool {
    token
        .split_once('=')
        .is_some_and(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_alphanumeric() || c == '_'))
}

/// Pair Bash calls with their results in timeline order.
pub fn analyze(events: &[Event]) -> Vec<CommandRun> {
    let mut pending: HashMap<&str, &str> = HashMap::new(); // call_id -> command
    let mut runs = Vec::new();

    for event in events {
        match event {
            Event::ToolCall {
                id, tool, input, ..
            } if tool == "Bash" => {
                if let Some(cmd) = input.0.get("command") {
                    pending.insert(id.0.as_str(), cmd.as_str());
                }
            }
            Event::ToolResult {
                call_id,
                outcome,
                ts,
                ..
            } => {
                if let Some(cmd) = pending.get(call_id.0.as_str()) {
                    runs.push(CommandRun {
                        ts: *ts,
                        command: cmd.to_string(),
                        kind: classify(cmd),
                        outcome: *outcome,
                    });
                }
            }
            _ => {}
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_test_and_build_invocations_classify() {
        assert_eq!(classify("cargo test 2>&1 | tail -8"), CommandKind::Test);
        assert_eq!(classify("RUST_BACKTRACE=1 cargo test"), CommandKind::Test);
        assert_eq!(classify("npm run test:ci"), CommandKind::Test);
        assert_eq!(classify("python -m pytest"), CommandKind::Test);
        assert_eq!(classify("./scripts/run_tests.sh"), CommandKind::Test);
        assert_eq!(classify("make test"), CommandKind::Test);
        assert_eq!(classify("cargo build --release"), CommandKind::Build);
        assert_eq!(classify("make"), CommandKind::Build);
        assert_eq!(classify("gcc -o out main.c"), CommandKind::Build);
        assert_eq!(classify("git status"), CommandKind::Other);
    }

    #[test]
    fn test_prose_in_commit_messages_and_echo_does_not_classify() {
        // The real-session false positive: a commit whose heredoc message mentions tests.
        let commit = "git add -A\ngit commit -q -m \"$(cat <<'EOF'\nFix parser\n\nAll tests pass after this change.\nEOF\n)\"";
        assert_eq!(classify(commit), CommandKind::Other);
        assert_eq!(classify("echo \"tests pass\""), CommandKind::Other);
        assert_eq!(
            classify("git commit -m 'make the build green'"),
            CommandKind::Other
        );
    }

    #[test]
    fn a_test_segment_anywhere_in_a_compound_command_wins() {
        assert_eq!(
            classify("echo \"===== cargo test =====\" && cargo test && cargo clippy"),
            CommandKind::Test
        );
        assert_eq!(
            classify("cargo test 2>&1 | grep -E 'test result|error'"),
            CommandKind::Test
        );
    }
}
