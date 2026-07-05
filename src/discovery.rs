//! Discovery — resolve which session(s) to audit.
//!
//! Encodes the cwd the way Claude Code encodes it for `~/.claude/projects/<encoded-cwd>/`,
//! selects sessions, filters noise (warmup, `/clear`-only). Pure path logic — no parsing.
//! See `docs/architecture.md` §1. Implemented in M1.
