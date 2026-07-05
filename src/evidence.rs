//! Evidence provider — the verifier's only window onto reality outside the transcript.
//!
//! Git via `std::process::Command` + filesystem checks. Trait-based for mocking in tests.
//! This trait plus transcript reading is the tool's ENTIRE I/O surface. No network, ever.
//! See `docs/architecture.md` §4. Implemented in M1.

use std::path::Path;

pub trait Evidence {
    fn file_exists(&self, p: &Path) -> bool;
    // diff_stat / commits_since land in M1 alongside concrete git + timestamp types.
}
