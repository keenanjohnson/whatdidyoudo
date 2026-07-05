# CLAUDE.md — whatdidyoudo

Rust CLI that audits completed AI coding agent sessions. Reads Claude Code JSONL transcripts from `~/.claude/projects/`, cross-references the agent's claims against its actual tool calls / exit codes / git state, prints a trust report. Binary name: `wdyd`. Crate name: `whatdidyoudo`.

Read `docs/architecture.md` before writing code. Read `docs/roadmap.md` for what to build next.

## Hard rules

- **No network calls anywhere in the runtime.** No telemetry, no update checks, no API calls. This is a core product promise, not a preference.
- **No async runtime.** Sequential file processing; `tokio` is banned as dependency weight.
- **Never load a whole JSONL file into memory.** Stream line-by-line with `BufReader`. Sessions can be very large.
- **Never panic on malformed input.** Unknown/unparseable lines become `Event::Unknown { raw }`. A Claude Code format change must degrade the report, never break the tool.
- **Only the ingestion layer may know the raw JSONL schema.** Everything downstream consumes the normalized `Event` timeline. Do not leak `serde_json::Value` past the adapter boundary.
- **Claims extraction and claims verification stay separate.** The extractor pattern-matches assistant text into `Claim` values and knows nothing about truth. The verifier matches claims against evidence and knows nothing about regexes.
- **Timestamps are the spine.** Normalize to UTC instants at ingestion. All claim-before-evidence ordering and git-log windowing depends on them. Never compare timestamp strings.
- **Shell out to `git` via `std::process::Command`; do not use libgit2.** More robust across odd repo states.

## Conventions

- Crates: `clap` (derive), `serde`/`serde_json`, `walkdir`, `regex`, `comfy-table`, `owo-colors`, `anyhow` (bin) / `thiserror` (lib), `insta` (dev, snapshot tests), `cargo-dist` (release).
- Structure as a lib + thin bin: analysis logic in `src/lib.rs` modules, CLI in `src/main.rs`.
- Analyzers are pure functions over `&[Event]` — no I/O. All I/O beyond reading the transcript goes through the `Evidence` trait (git + filesystem) so tests can mock it.
- Every parser change requires a fixture: add an anonymized JSONL sample to `fixtures/` with an `insta` snapshot of the expected `AuditReport`. Fixtures are the regression net against format churn.
- Before claiming tests pass, run `cargo test` and show the output. Before claiming the build works, run `cargo build`. (Yes, this project holds you to its own standard.)
- Keep diffs scoped to the task. No new dependencies without justification in the PR/commit message. No new abstractions used only once.

## Domain notes

- Claude Code session transcripts: JSONL at `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`. Format is undocumented and version-dependent — treat every field as optional.
- Filter noise sessions: warmup transcripts and `/clear`-only sessions should not surface in discovery.
- Agents may commit mid-session: blast radius must walk `git log` from a pre-session ref, not just diff the working tree.
- Subagent transcripts can appear as separate entries/files belonging to one logical session.
- Claim taxonomy (v1): `TestsPass`, `BuildSucceeds`, `FileCreated(path)`, `BugFixed`, `Committed`. Verdicts: `Verified(evidence)`, `Unverified`, `Contradicted(evidence)`. Keep the taxonomy small and mechanically checkable; resist prose NLP.

## Commands

```bash
cargo build
cargo test
cargo insta review        # after intentional report-format changes
cargo clippy -- -D warnings
cargo fmt --check
```
