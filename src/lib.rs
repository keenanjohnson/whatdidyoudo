//! whatdidyoudo — audit completed AI coding agent sessions.
//!
//! Pipeline: discovery → ingestion → analyzers (+ evidence) → report.
//! See `docs/architecture.md`. Modules are stubs at M0; types define the seams.

pub mod analyzers;
pub mod discovery;
pub mod evidence;
pub mod ingestion;
pub mod report;

pub use discovery::Discovery;
pub use ingestion::{ClaudeCodeAdapter, Event, SourceAdapter, Timestamp, ToolOutcome};
pub use report::{AuditReport, SessionMeta};
