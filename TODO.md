# Pre-share TODO

Findings from running `wdyd` against this repo's own real 767-event build session
(`~/.claude/projects/-Users-keen-Documents-GitHub-whatdidyoudo/e4c5dc06-6df3-4b87-adbb-1c823d13b531.jsonl`).
Work top to bottom: the verdict bugs are brand-killers for a trust tool; polish and infra follow.

Per CLAUDE.md, every verdict-bug fix needs a fixture in `fixtures/` + an `insta` snapshot reproducing the false verdict before the fix.

## 1. Verdict bugs (false accusations / false confirmations)

- [ ] **False `CONTRADICTED: created discovery.rs`** — `file_verdict` in `src/analyzers/claims.rs` matches the session write suffix-tolerantly (`/…/src/discovery.rs` ends with `discovery.rs`) but then checks disk existence with the literal relative path, which isn't at the repo root. Fix: when a session write matched, check existence of the *matched write path*, not the raw claimed path.
- [ ] **False `CONTRADICTED: committed`** — extractor regex `\bcommitted\b` (`src/analyzers/claims.rs`) fired on a status recap ("M0 skeleton — committed … **but not committed**") referring to a previous session. Add a negation/context guard, and/or downgrade "no commit found in window" to `Unverified` — absence of a commit isn't proof of a lie.
- [ ] **False `VERIFIED: tests pass`** — `classify` in `src/analyzers/commands.rs` marks any command containing the substring "test" as a test run, including a `git commit` whose heredoc message mentions tests. Fix: classify only actual program invocations (first token per pipeline segment / known runners like `cargo test`, `pytest`), not echo text or commit messages.

## 2. Output polish

- [ ] **`--md` table breaks on GitHub** — evidence cells contain raw newlines, which terminate a GFM table row (pipes are escaped, newlines aren't). "PR-paste-ready" is the core M2 promise.
- [ ] **Truncate evidence strings** — full multi-line heredocs (entire commit messages) get dumped into the table in both terminal and markdown. Truncate to first line, ~80 chars.
- [ ] **Show the agent's quote for contradicted claims** — the extractor captures `quote` but the report drops it everywhere, including `--json`. An accusation needs to show what the agent actually said.
- [ ] **Scope heuristic reads as hostile** — the session that legitimately built this repo scored scope 0% because no user message named the files. Loosen the heuristic or present scope neutrally when the user gave a broad task.

## 3. Infra before sharing

- [ ] **CI** — `cargo test` + `clippy -D warnings` + `fmt --check` on push (GitHub Actions). Cheap; do first among infra.
- [ ] **Reserve crates.io names** — publish placeholder `whatdidyoudo` + `wdyd` (manual crates.io action).
- [ ] **Homebrew tap** — create the `homebrew-tap` repo + tap-push token secret before the first `cargo-dist` release tag.

## Optional (fine to ship without)

- [ ] `--last N` / `--since` session-selection flags
- [ ] README recipes: pre-commit hook, CI, Claude Code Stop hook
- [ ] Hygiene analyzer (TODOs, debug prints)
