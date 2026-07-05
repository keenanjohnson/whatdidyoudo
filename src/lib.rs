//! whatdidyoudo — audit completed AI coding agent sessions.
//!
//! Pipeline: discovery → ingestion → analyzers (+ evidence) → report.
//! See `docs/architecture.md`. Modules are stubs at M0; types define the seams.

pub mod analyzers;
pub mod discovery;
pub mod evidence;
pub mod ingestion;
pub mod report;

pub use ingestion::{Event, Timestamp};
