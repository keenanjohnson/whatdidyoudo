//! Ingestion — the schema firewall.
//!
//! The ONLY code that knows the raw Claude Code JSONL format. Streams line-by-line,
//! deserializes permissively, and emits the normalized [`Event`] timeline. Nothing
//! downstream may see `serde_json::Value`. See `docs/architecture.md` §2.

use std::io::BufRead;
use std::path::Path;

/// UTC instant. Timestamps are the spine — every ordering decision depends on them.
/// Newtype now so we never accidentally compare raw strings; concrete repr lands in M1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub i64); // unix millis, placeholder repr

/// Opaque id linking a `ToolCall` to its `ToolResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallId(pub String);

/// Normalized timeline event. Malformed input degrades to [`Event::Unknown`] —
/// never dropped, never a panic.
#[derive(Debug, Clone)]
pub enum Event {
    UserMessage { ts: Timestamp, text: String },
    AssistantText { ts: Timestamp, text: String },
    ToolCall { ts: Timestamp, id: CallId, tool: String, input: String },
    ToolResult { ts: Timestamp, call_id: CallId, exit_code: Option<i32>, output: String },
    Unknown { ts: Option<Timestamp>, raw: serde_json::Value },
}

/// A source of agent transcripts. `ClaudeCodeAdapter` first; Codex/Cursor later,
/// touching nothing downstream.
pub trait SourceAdapter {
    fn detect(path: &Path) -> bool;
    fn parse<R: BufRead>(reader: R) -> Box<dyn Iterator<Item = Event>>;
}

// ClaudeCodeAdapter lands in M1, built against real fixtures — not assumptions.
