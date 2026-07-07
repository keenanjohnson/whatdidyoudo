# Pre-share TODO

Findings from running `wdyd` against this repo's own real 767-event build session
(`~/.claude/projects/-Users-keen-Documents-GitHub-whatdidyoudo/e4c5dc06-6df3-4b87-adbb-1c823d13b531.jsonl`).
Work top to bottom: the verdict bugs are brand-killers for a trust tool; polish and infra follow.

Per CLAUDE.md, every verdict-bug fix needs a fixture in `fixtures/` + an `insta` snapshot reproducing the false verdict before the fix.

## 1. Verdict bugs (false accusations / false confirmations)

- [x] **False `CONTRADICTED: created discovery.rs`** — fixed: `file_verdict` now checks disk existence of the *matched write path* (often absolute), not the bare claimed name. Regression test: `relative_claim_verifies_against_the_absolute_write_path`.
- [x] **False `CONTRADICTED: committed`** — fixed both halves: the extractor skips negated mentions ("not committed yet"), and "no commit found in window" is now `Unverified`, not `Contradicted` (the claim may recap an earlier session; log windowing can miss rebases/amends).
- [x] **False `VERIFIED: tests pass`** — fixed: `classify` is now structural — heredoc bodies stripped, commands split into shell segments, only the program word (plus launcher sub-command like `cargo test`) classifies. Prose in `echo`/commit messages can't count as a test run.
- [x] **Extractor captures "i.e." as a filename** — fixed: abbreviation-shaped captures (`i.e`, `e.g`) are skipped, and the extractor scans past them so a real filename later in the same message still extracts. Regression tests: `prose_abbreviations_are_not_filenames`, `abbreviation_does_not_shadow_a_real_filename`, plus the "i.e." prose planted in `fixtures/session_false_verdicts.jsonl`.

## 2. Output polish

- [x] **`--md` table breaks on GitHub** — fixed: `md_escape` now also strips `\r` and turns `\n` into `<br>`, so no cell content can terminate a GFM row. Regression test: `markdown_rows_survive_multiline_evidence`.
- [x] **Truncate evidence strings** — fixed: table cells (terminal + markdown) show the first line capped at 80 chars with `…`; `--json` keeps the full string. Regression test: `evidence_is_truncated_in_tables_but_not_json`.
- [x] **Show the agent's quote for contradicted claims** — fixed: the quote is now the *line* that triggered the claim (not the whole message), contradicted rows show `agent said: "…"` in terminal + markdown, and `--json` carries `quote` on every claim. Regression tests: `contradicted_claims_show_the_agents_words`, `quote_is_the_claim_line_not_the_whole_message`.
- [x] **Scope heuristic reads as hostile** — fixed: when no user message names a file/path (`BlastRadius::broad_task`), scope renders as `n/a (broad task)`, per-file scope shows `—` instead of red `OUT OF SCOPE`, and `--json` emits `null` for `scope_pct`/`in_scope`. Regression tests: `broad_task_scope_is_presented_neutrally`, `task_naming_no_files_is_broad`.
- [ ] **Evidence for compound commands truncates to the boring prefix** — `cd /Users/keen/… && cargo test` truncates to `` `cd /Users/keen/Documents/GitHub/whatdidyoudo…` ``, hiding the part that mattered. Fix: have the commands analyzer report the shell *segment* that decided the classification (`cargo test`) and use it as the evidence string in `command_verdict`. Small refactor in `commands.rs` (`classify` needs to return the decisive segment); watch out that `segments()` blanks quoted spans and splits on `&`, so the raw segment may need cleanup before display.

## 3. Infra before sharing

- [ ] **CI** — `cargo test` + `clippy -D warnings` + `fmt --check` on push (GitHub Actions). Cheap; do first among infra.
- [ ] **Reserve crates.io names** — publish placeholder `whatdidyoudo` + `wdyd` (manual crates.io action).
- [ ] **Homebrew tap** — create the `homebrew-tap` repo + tap-push token secret before the first `cargo-dist` release tag.

## Optional (fine to ship without)

- [ ] `--last N` / `--since` session-selection flags
- [ ] README recipes: pre-commit hook, CI, Claude Code Stop hook
- [ ] Hygiene analyzer (TODOs, debug prints)
