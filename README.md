# whatdidyoudo

> Your agent says "Done! All tests pass." Did they?

`wdyd` is a one-command, after-the-fact audit for AI coding agent sessions. It reads the session transcript your agent already wrote to disk, cross-references every claim the agent made against what it actually did, and prints a one-page trust report.

**No wrapper. No proxy. No config. No network.** Run it on a session that already happened.

```
$ wdyd

Session: "refactor auth module" · 47 min · finished 6 min ago

BLAST RADIUS                          CLAIMS
  14 files changed (+412 −208)          ✓ "build succeeds"      cargo build, exit 0
  ⚠ 5 files outside src/auth/           ✗ "all tests pass"      NO TEST COMMAND EVER RAN
    src/billing/invoice.rs  +89         ✓ "created auth_mw.rs"  file exists, Write call

DEPENDENCIES                          LEFT BEHIND
  + tokio-util 0.7 (not discussed)      3 TODOs, 1 dbg!() in invoice.rs

Trust summary: 2 verified · 1 UNVERIFIED · scope compliance 64%
```

## Why

Coding agents generate completion language regardless of the actual state of the codebase. They will write "tests passing" while the suite has syntax errors, and claim files exist that were never written. The industry answer so far is heavyweight: orchestration platforms, MCP gateways, CI verification layers — all of which require changing how you run your agent *before* they deliver value.

`wdyd` takes the opposite bet: the evidence is already on your disk. Claude Code writes a full JSONL transcript of every session — every tool call, every command, every exit code. Auditing it should be as cheap as `git status`.

## Install

```bash
cargo install whatdidyoudo        # installs the `wdyd` binary
# or
brew install whatdidyoudo         # (planned)
# or grab a prebuilt binary from Releases
```

## Usage

```bash
wdyd                    # audit the most recent session in this project
wdyd --last 3           # audit the last three sessions
wdyd --session <id>     # audit a specific session
wdyd --since monday     # list sessions since a date with one-line trust summaries
wdyd --md               # markdown output, ready to paste into a PR description
wdyd --json             # machine-readable, for scripting
wdyd --check            # exit non-zero on unverified/contradicted claims — for hooks & CI
```

The `--check` flag composes with git pre-commit hooks, CI jobs, and Claude Code Stop hooks so the audit runs automatically when a session ends. See `docs/user-experience.md` for recipes.

## What it checks

- **Blast radius** — every file the agent touched (from tool calls, cross-checked against git), annotated in-scope / out-of-scope relative to the task.
- **Claims vs. evidence** — assertions like "tests pass", "build succeeds", "created X", "committed" are extracted from the agent's own words and matched against the commands it actually ran and their exit codes. Verdicts: `verified`, `unverified` (claim made, nothing ever ran), `contradicted` (it ran, it failed, the claim was made anyway).
- **Dependencies** — packages installed mid-session, flagged if never discussed.
- **Hygiene** — TODOs, debug prints, and other droppings left in the diff.

## Principles

1. **Read-only and retroactive.** Zero changes to how you run your agent. Value on the first run, against history you already have.
2. **Offline by default, forever.** No API calls, no telemetry, no accounts. The tool that audits your agent should not itself be a supply-chain question mark.
3. **Evidence over narration.** An agent saying it did something is not evidence. Exit codes, diffs, and files on disk are.
4. **Degrade, never crash.** The transcript format is undocumented and churns; unknown content becomes an `Unknown` event, not a panic.

## Status

Pre-alpha. Claude Code (local sessions) is the first supported agent; the parser is behind a `SourceAdapter` trait so Codex / Cursor / others can follow. See `docs/roadmap.md`.

## License

MIT
