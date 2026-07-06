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

/// Rough command classification via substring tables. Deliberately small and mechanical.
pub fn classify(command: &str) -> CommandKind {
    let c = command.to_lowercase();
    if c.contains("test") || c.contains("pytest") || c.contains("jest") || c.contains("vitest") {
        CommandKind::Test
    } else if c.contains("build") || c.contains("compile") || c.contains("make ") || c == "make" {
        CommandKind::Build
    } else {
        CommandKind::Other
    }
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
