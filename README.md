# whatdidyoudo

> Your agent says *"Done! All tests pass."* Did it?

`wdyd` is a one-command, after-the-fact audit for AI coding agent sessions. It reads the transcript your agent already wrote to disk, cross-references every claim it made against what it **actually did** — the commands it ran, their exit codes, the files it changed, the commits it made — and prints a one-page trust report.

<p align="center">
  <img src="assets/demo.gif" alt="wdyd auditing a session: 'tests pass' contradicted, build/file/commit verified, an out-of-scope shell edit caught by git" width="820">
</p>

**No wrapper. No proxy. No config. No network.** Point it at a session that already happened.

## Why

Coding agents produce completion language regardless of the actual state of the code. They'll write *"tests passing"* while the suite is red, and claim files exist that were never written. The common answers are heavyweight — orchestration platforms, MCP gateways, CI layers — all of which change how you run your agent *before* they deliver any value.

`wdyd` takes the opposite bet: **the evidence is already on your disk.** Claude Code writes a full JSONL transcript of every session — every tool call, every command, every result. Auditing it should be as cheap as `git status`.

## Install

```bash
cargo install whatdidyoudo        # installs the `wdyd` binary
```

Or grab a prebuilt binary from [Releases](https://github.com/keenanjohnson/whatdidyoudo/releases). Homebrew tap coming with the first tagged release.

## Usage

```bash
wdyd                              # audit the latest session in this project
wdyd --session path/to.jsonl      # audit a specific transcript
wdyd --format md                  # Markdown, ready to paste into a PR
wdyd --format json                # machine-readable, for scripting
wdyd --check                      # exit non-zero if any claim is CONTRADICTED
```

Run `wdyd` from inside the repo the session belongs to — that's how it reaches the git history and files to verify against.

`--check` composes with git pre-commit hooks and CI so the audit runs automatically:

```bash
wdyd --check || echo "an agent claim was contradicted — look before you merge"
```

## What it checks

- **Claims vs. evidence** — assertions like *tests pass*, *build succeeds*, *created X*, and *committed* are extracted from the agent's own words and matched against reality:
  - `VERIFIED` — the evidence backs it (the test command exited 0; the file is on disk; a commit exists in the session window).
  - `CONTRADICTED` — the evidence refutes it (the build command *failed*, yet success was claimed).
  - `UNVERIFIED` — the claim was made but nothing corroborates or refutes it.
- **Blast radius** — every file the agent touched, in-scope / out-of-scope relative to the task. Cross-checked against git, so edits made via `sed` or shell — not just `Write`/`Edit` — are caught too (shown as `shell/git`).
- **Trust summary** — one line: `N verified · M unverified · K contradicted · scope X%`.

Dependency and hygiene (leftover TODOs / debug prints) analyzers are on the roadmap.

## How it works

```
discovery → ingestion → analyzers (+ git/fs evidence) → report
find the    JSONL →     blast radius · commands ·        terminal /
session     Event       claims extract + verify          md / json
            timeline
```

The extractor pattern-matches the agent's prose into a small, mechanically-checkable claim taxonomy and knows nothing about truth. The verifier matches those claims against evidence and knows nothing about how they were extracted. The two never share a line — so a missed claim is a regex gap, and a wrong verdict is a logic bug, and you always know which.

## Principles

1. **Read-only and retroactive.** Zero changes to how you run your agent. Value on the first run, against history you already have.
2. **Offline by default, forever.** No API calls, no telemetry, no accounts. The tool that audits your agent shouldn't itself be a supply-chain question mark.
3. **Evidence over narration.** An agent saying it did something is not evidence. Exit codes, diffs, and commits are.
4. **Degrade, never crash.** The transcript format is undocumented and churns; unknown content becomes an `Unknown` event, not a panic.

## Status

Pre-alpha, but the core works end-to-end. Claude Code (local sessions) is the first supported agent; the parser sits behind a `SourceAdapter` trait so Codex / Cursor / others can follow. See [`docs/roadmap.md`](docs/roadmap.md) and [`docs/architecture.md`](docs/architecture.md).

## License

MIT OR Apache-2.0
