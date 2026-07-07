# Pre-share TODO

Findings from running `wdyd` against this repo's own real 767-event build session
(`~/.claude/projects/-Users-keen-Documents-GitHub-whatdidyoudo/e4c5dc06-6df3-4b87-adbb-1c823d13b531.jsonl`).
Work top to bottom: the verdict bugs are brand-killers for a trust tool; polish and infra follow.

Per CLAUDE.md, every verdict-bug fix needs a fixture in `fixtures/` + an `insta` snapshot reproducing the false verdict before the fix.

## 1. Verdict bugs (false accusations / false confirmations)

- [x] **False `CONTRADICTED: created discovery.rs`** — fixed: `file_verdict` now checks disk existence of the *matched write path* (often absolute), not the bare claimed name. Regression test: `relative_claim_verifies_against_the_absolute_write_path`.
- [x] **False `CONTRADICTED: committed`** — fixed both halves: the extractor skips negated mentions ("not committed yet"), and "no commit found in window" is now `Unverified`, not `Contradicted` (the claim may recap an earlier session; log windowing can miss rebases/amends).
- [x] **False `VERIFIED: tests pass`** — fixed: `classify` is now structural — heredoc bodies stripped, commands split into shell segments, only the program word (plus launcher sub-command like `cargo test`) classifies. Prose in `echo`/commit messages can't count as a test run.
- [ ] **Extractor captures "i.e." as a filename** — found while re-auditing after the fixes above: `file_created_re` matched prose containing "i.e" as `FileCreated("i.e")` (harmlessly `Unverified`, but sloppy). Exclude abbreviation-shaped captures (`i.e`, `e.g`) or require a known file extension.

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
