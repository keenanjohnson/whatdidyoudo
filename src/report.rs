//! Report engine — every analyzer feeds one `AuditReport`; renderers consume only it.
//!
//! terminal (comfy-table + owo-colors) · `--md` · `--json` · `--check` exit code.
//! See `docs/architecture.md` §5. Fields fill in as analyzers land across M1/M2.

/// The single struct all analyzers feed and all renderers consume.
#[derive(Debug, Default)]
pub struct AuditReport {
    // session, blast_radius, claims, dependencies, hygiene, trust_summary — added per analyzer.
}
