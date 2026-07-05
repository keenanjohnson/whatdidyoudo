//! Analyzers — pure passes over `&[Event]`. No I/O; unit-testable against fixtures.
//! See `docs/architecture.md` §3.
//!
//! Extraction and verification stay deliberately separate: extraction failures are
//! regex gaps (patch with fixtures); verification failures are logic bugs.

pub mod claims;

// blast_radius, commands, dependencies, hygiene land in M1/M2.

/// The v1 claim taxonomy. Small and mechanically checkable by design — resist prose NLP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClaimKind {
    TestsPass,
    BuildSucceeds,
    FileCreated(std::path::PathBuf),
    BugFixed,
    Committed,
}

/// Result of verifying a claim against evidence dated before the claim's timestamp.
#[derive(Debug, Clone)]
pub enum Verdict {
    Verified,     // + EvidenceRef in M2
    Unverified,
    Contradicted, // + EvidenceRef in M2
}
