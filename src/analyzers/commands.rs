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
    /// The shell segment that decided `kind` (`cargo test` out of `cd … && cargo test`).
    /// Evidence strings show this so the part that mattered survives display truncation.
    /// Falls back to the whole command when no segment decided (kind `Other`).
    pub decisive: String,
    pub kind: CommandKind,
    pub outcome: ToolOutcome,
}

/// Rough command classification. Deliberately small and mechanical, but structural:
/// only the program word of each shell segment (plus a launcher's sub-command, e.g.
/// `cargo test`) can classify. Prose in heredoc bodies, `echo` strings, or commit
/// messages never counts — a `git commit -m "tests pass"` is not a test run.
///
/// Also returns the segment that decided the kind, cleaned up for display; `None`
/// when the command is `Other`.
pub fn classify(command: &str) -> (CommandKind, Option<String>) {
    let mut kind = CommandKind::Other;
    let mut decisive = None;
    for (blanked, raw) in segments(command) {
        match classify_segment(&blanked) {
            CommandKind::Test => return (CommandKind::Test, Some(clean_segment(&raw))),
            CommandKind::Build => {
                if kind == CommandKind::Other {
                    kind = CommandKind::Build;
                    decisive = Some(clean_segment(&raw));
                }
            }
            CommandKind::Other => {}
        }
    }
    (kind, decisive)
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

/// Split a command into shell segments: heredoc bodies dropped, quoted spans blanked,
/// then split on newlines and `|`, `;`, `&` (which also covers `&&` / `||` via empty
/// segments). Quotes are blanked first so prose inside a quoted argument can't form a
/// segment — `git commit -m "foo | ctest"` must stay one `git` segment. Each segment
/// comes back as (blanked, raw): classification reads the blanked text, evidence
/// display gets the raw text with its quotes intact.
fn segments(command: &str) -> Vec<(String, String)> {
    let mut kept = Vec::new();
    let mut terminator: Option<&str> = None;
    for line in command.lines() {
        if let Some(t) = terminator {
            if line.trim() == t {
                terminator = None;
            }
            continue;
        }
        terminator = heredoc_terminator(line);
        kept.push(line);
    }
    let raw = kept.join("\n");
    let blanked = strip_quoted(&raw);

    // `strip_quoted` is byte-length-preserving, and every delimiter it leaves standing
    // is outside quotes (an ASCII byte at the same offset in `raw`), so blanked spans
    // slice `raw` at valid boundaries.
    let mut out = Vec::new();
    let mut start = 0;
    for (i, b) in blanked.bytes().enumerate() {
        if matches!(b, b'\n' | b'|' | b';' | b'&') {
            out.push((blanked[start..i].to_string(), raw[start..i].to_string()));
            start = i + 1;
        }
    }
    out.push((blanked[start..].to_string(), raw[start..].to_string()));
    out
}

/// Tidy a raw decisive segment for display: trim, and drop a dangling redirection stub
/// left where splitting ate the `&1` of `2>&1` (`cargo test 2>` → `cargo test`).
fn clean_segment(raw: &str) -> String {
    let mut s = raw.trim();
    while let Some((rest, last)) = s.rsplit_once(char::is_whitespace) {
        let bare = last.trim_start_matches(|c: char| c.is_ascii_digit());
        if !bare.is_empty() && bare.chars().all(|c| c == '>') {
            s = rest.trim_end();
        } else {
            break;
        }
    }
    s.to_string()
}

/// Blank single- and double-quoted spans with spaces, preserving byte length so
/// offsets map back into the input. A quote may span lines (real newlines inside a
/// quoted commit message). Backslash escapes count only inside double quotes,
/// matching shell rules.
fn strip_quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in text.chars() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' && q == '"' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
                for _ in 0..c.len_utf8() {
                    out.push(' ');
                }
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                out.push(' ');
            }
            None => out.push(c),
        }
    }
    out
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
                    let (kind, decisive) = classify(cmd);
                    runs.push(CommandRun {
                        ts: *ts,
                        command: cmd.to_string(),
                        decisive: decisive.unwrap_or_else(|| cmd.to_string()),
                        kind,
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

    fn kind(command: &str) -> CommandKind {
        classify(command).0
    }

    #[test]
    fn real_test_and_build_invocations_classify() {
        assert_eq!(kind("cargo test 2>&1 | tail -8"), CommandKind::Test);
        assert_eq!(kind("RUST_BACKTRACE=1 cargo test"), CommandKind::Test);
        assert_eq!(kind("npm run test:ci"), CommandKind::Test);
        assert_eq!(kind("python -m pytest"), CommandKind::Test);
        assert_eq!(kind("./scripts/run_tests.sh"), CommandKind::Test);
        assert_eq!(kind("make test"), CommandKind::Test);
        assert_eq!(kind("cargo build --release"), CommandKind::Build);
        assert_eq!(kind("make"), CommandKind::Build);
        assert_eq!(kind("gcc -o out main.c"), CommandKind::Build);
        assert_eq!(kind("git status"), CommandKind::Other);
    }

    #[test]
    fn test_prose_in_commit_messages_and_echo_does_not_classify() {
        // The real-session false positive: a commit whose heredoc message mentions tests.
        let commit = "git add -A\ngit commit -q -m \"$(cat <<'EOF'\nFix parser\n\nAll tests pass after this change.\nEOF\n)\"";
        assert_eq!(kind(commit), CommandKind::Other);
        assert_eq!(kind("echo \"tests pass\""), CommandKind::Other);
        assert_eq!(
            kind("git commit -m 'make the build green'"),
            CommandKind::Other
        );
    }

    #[test]
    fn quoted_prose_cannot_start_a_segment() {
        // A `|` inside a quoted argument is not a pipe — the words after it must not
        // classify ("ctest" would otherwise read as a test runner).
        assert_eq!(kind("git commit -m \"foo | ctest\""), CommandKind::Other);
        assert_eq!(kind("echo 'lint; make; test'"), CommandKind::Other);
        assert_eq!(
            kind("git commit -m \"multi line | first\ntest everything works\""),
            CommandKind::Other
        );
    }

    #[test]
    fn a_test_segment_anywhere_in_a_compound_command_wins() {
        assert_eq!(
            kind("echo \"===== cargo test =====\" && cargo test && cargo clippy"),
            CommandKind::Test
        );
        assert_eq!(
            kind("cargo test 2>&1 | grep -E 'test result|error'"),
            CommandKind::Test
        );
    }

    #[test]
    fn decisive_segment_is_the_part_that_classified() {
        // The TODO case: a `cd &&` prefix must not hide the command that mattered.
        assert_eq!(
            classify("cd /Users/someone/very/long/project/path && cargo test 2>&1 | tail -8"),
            (CommandKind::Test, Some("cargo test".into()))
        );
        // Quoted arguments survive in the raw segment.
        assert_eq!(
            classify("cargo test --features \"foo bar\" && git status"),
            (
                CommandKind::Test,
                Some("cargo test --features \"foo bar\"".into())
            )
        );
        // A simple command is its own decisive segment.
        assert_eq!(
            classify("cargo build --release"),
            (CommandKind::Build, Some("cargo build --release".into()))
        );
        // `Other` has no decisive segment.
        assert_eq!(classify("git status"), (CommandKind::Other, None));
    }
}
