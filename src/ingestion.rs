//! Ingestion — the schema firewall.
//!
//! The ONLY code that knows the raw Claude Code JSONL format. Streams line-by-line
//! (`BufRead`), deserializes permissively, and emits the normalized [`Event`] timeline.
//! No `serde_json::Value` crosses this boundary except inside [`Event::Unknown`].
//! See `docs/architecture.md` §2.

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// UTC instant. Timestamps are the spine — every ordering decision depends on them.
/// Parsed once at ingestion; never compared as strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(pub OffsetDateTime);

impl Timestamp {
    fn parse(s: &str) -> Option<Self> {
        OffsetDateTime::parse(s, &Rfc3339).ok().map(Timestamp)
    }

    /// RFC 3339 rendering for machine-readable output (`--json`).
    pub fn to_rfc3339(&self) -> String {
        self.0.format(&Rfc3339).unwrap_or_default()
    }
}

/// Opaque id linking a `ToolCall` to its `ToolResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallId(pub String);

/// Normalized tool input: the call's fields flattened to strings. Keeps
/// `serde_json::Value` out of the downstream API while losing nothing analyzers need
/// (e.g. `"command"` for Bash, `"file_path"` for Write/Edit).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolInput(pub BTreeMap<String, String>);

/// Whether a tool call succeeded. The transcript records no numeric exit code, so this
/// is derived from the `tool_result` block's `is_error` plus the line's `interrupted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolOutcome {
    Ok,
    Failed,
    Interrupted,
    Unknown,
}

/// Normalized timeline event. Malformed input degrades to [`Event::Unknown`] —
/// never dropped, never a panic.
#[derive(Debug, Clone)]
pub enum Event {
    UserMessage {
        ts: Timestamp,
        text: String,
    },
    AssistantText {
        ts: Timestamp,
        text: String,
    },
    ToolCall {
        ts: Timestamp,
        id: CallId,
        tool: String,
        input: ToolInput,
    },
    ToolResult {
        ts: Timestamp,
        call_id: CallId,
        outcome: ToolOutcome,
        output: String,
    },
    Unknown {
        ts: Option<Timestamp>,
        raw: serde_json::Value,
    },
}

/// A source of agent transcripts. `ClaudeCodeAdapter` first; Codex/Cursor later,
/// touching nothing downstream.
pub trait SourceAdapter {
    fn detect(path: &Path) -> bool;
    fn parse<R: BufRead>(reader: R) -> impl Iterator<Item = Event>;
}

/// Claude Code JSONL adapter. See module docs for the confirmed schema.
pub struct ClaudeCodeAdapter;

impl SourceAdapter for ClaudeCodeAdapter {
    fn detect(path: &Path) -> bool {
        path.extension().is_some_and(|e| e == "jsonl")
    }

    fn parse<R: BufRead>(reader: R) -> impl Iterator<Item = Event> {
        // Stream: one raw line -> zero or more events. The file is never fully buffered;
        // only the growing Event timeline is retained (analyzers need `&[Event]`).
        reader
            .lines()
            .map_while(Result::ok)
            .flat_map(|line| parse_line(&line))
    }
}

/// Parse one JSONL line into its events. Any failure degrades to a single
/// `Unknown` that preserves the raw line — this function never panics, never drops.
fn parse_line(line: &str) -> Vec<Event> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    // Parse to Value once: content-bearing lines become events; everything else
    // (and any non-JSON line) is retained verbatim as `Unknown`.
    match serde_json::from_str::<serde_json::Value>(trimmed) {
        Ok(value) => line_to_events(value),
        Err(_) => vec![Event::Unknown {
            ts: None,
            raw: serde_json::Value::String(trimmed.to_owned()),
        }],
    }
}

fn line_to_events(value: serde_json::Value) -> Vec<Event> {
    let ts = value
        .get("timestamp")
        .and_then(serde_json::Value::as_str)
        .and_then(Timestamp::parse);

    let content = value.get("message").and_then(|m| m.get("content")).cloned();

    // Only lines with a timestamp AND message content sit on the timeline; the rest
    // (system / mode / ai-title / …) are preserved as Unknown, not invented onto the spine.
    let (Some(ts), Some(content)) = (ts, content) else {
        return vec![Event::Unknown { ts, raw: value }];
    };

    let interrupted = value
        .get("toolUseResult")
        .and_then(|v| v.get("interrupted"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    match content {
        serde_json::Value::String(text) => vec![Event::UserMessage { ts, text }],
        serde_json::Value::Array(_) => match serde_json::from_value::<Vec<RawBlock>>(content) {
            Ok(blocks) => blocks
                .into_iter()
                .filter_map(|b| block_to_event(b, ts, interrupted))
                .collect(),
            Err(_) => vec![Event::Unknown {
                ts: Some(ts),
                raw: value,
            }],
        },
        _ => vec![Event::Unknown {
            ts: Some(ts),
            raw: value,
        }],
    }
}

// ---- Raw block shape (permissive; every field optional) -------------------------

#[derive(Deserialize)]
struct RawBlock {
    #[serde(default, rename = "type")]
    kind: String,
    // text block
    #[serde(default)]
    text: Option<String>,
    // tool_use block
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    // tool_result block
    #[serde(default)]
    tool_use_id: Option<String>,
    #[serde(default)]
    is_error: Option<bool>,
    #[serde(default)]
    content: Option<serde_json::Value>,
}

fn block_to_event(b: RawBlock, ts: Timestamp, interrupted: bool) -> Option<Event> {
    match b.kind.as_str() {
        "text" => Some(Event::AssistantText {
            ts,
            text: b.text.unwrap_or_default(),
        }),
        "tool_use" => Some(Event::ToolCall {
            ts,
            id: CallId(b.id.unwrap_or_default()),
            tool: b.name.unwrap_or_default(),
            input: normalize_input(b.input),
        }),
        "tool_result" => Some(Event::ToolResult {
            ts,
            call_id: CallId(b.tool_use_id.unwrap_or_default()),
            outcome: outcome_of(b.is_error, interrupted),
            output: flatten_result_content(b.content),
        }),
        // `thinking` and any future block type are not auditable — drop silently.
        _ => None,
    }
}

fn outcome_of(is_error: Option<bool>, interrupted: bool) -> ToolOutcome {
    match (interrupted, is_error) {
        (true, _) => ToolOutcome::Interrupted,
        (false, Some(true)) => ToolOutcome::Failed,
        (false, Some(false)) => ToolOutcome::Ok,
        (false, None) => ToolOutcome::Unknown,
    }
}

/// Flatten a tool-call input object to `field -> string`. Non-string values keep their
/// compact JSON so nothing is lost; a non-object input lands under `"value"`.
fn normalize_input(input: Option<serde_json::Value>) -> ToolInput {
    let mut map = BTreeMap::new();
    match input {
        Some(serde_json::Value::Object(obj)) => {
            for (k, v) in obj {
                map.insert(k, value_to_string(&v));
            }
        }
        Some(other) => {
            map.insert("value".to_string(), value_to_string(&other));
        }
        None => {}
    }
    ToolInput(map)
}

/// A `tool_result` block's `content` is usually a string; occasionally an array of
/// `{type:"text", text}` parts. Join the text; fall back to compact JSON.
fn flatten_result_content(content: Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s,
        Some(serde_json::Value::Array(parts)) => parts
            .iter()
            .filter_map(|p| p.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
        None => String::new(),
    }
}

fn value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}
