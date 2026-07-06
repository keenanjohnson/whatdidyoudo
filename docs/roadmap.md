# Roadmap

## M0 — Skeleton (first session in Claude Code)
- [ ] `cargo new` workspace: lib + thin bin, binary named `wdyd`
- [ ] Reserve names: publish placeholder `whatdidyoudo` + `wdyd` crates on crates.io
- [ ] CI: `cargo test`, `clippy -D warnings`, `fmt --check` on push
- [ ] Collect 2–3 real JSONL transcripts into `fixtures/` (anonymized) — **build the parser against real data, not assumptions**

## M1 — Blast radius (working v1, ~weekend one)
- [x] Discovery: cwd → encoded project dir → latest session; noise filtering
- [x] `ClaudeCodeAdapter`: streaming permissive JSONL → `Event` timeline
- [x] Blast-radius analyzer (scope heuristic); command + dependency analyzers still TODO
- [ ] Git evidence provider (diff-stat, commits-since) — also the way to catch Bash/`sed`-based edits the Write/Edit analyzer misses
- [x] Terminal renderer; `--json`
- [x] `insta` snapshots for every fixture
- Milestone demo: `cargo install` → `wdyd` in a real project → real blast-radius report in <1s

## M2 — Claims table (the product, ~weekend two)
- [x] Claims extractor: `TestsPass`, `BuildSucceeds`, `FileCreated` (+ `BugFixed`, `Committed` extraction TODO)
- [x] Claims verifier with `Verified` / `Unverified` / `Contradicted` + evidence strings (`Committed` needs git; `BugFixed` unverifiable)
- [x] Trust summary + scope-compliance %
- [x] `--md` renderer (PR-paste-ready), `--check` exit codes
- [ ] Hygiene analyzer (TODOs, debug prints)
- [ ] README with real screenshot + 30-second demo GIF at top

## M3 — Launch
- [ ] `cargo-dist`: prebuilt binaries, install script, Homebrew tap
- [ ] `--last N`, `--session`, `--since`
- [ ] README recipes: pre-commit hook, CI, Claude Code Stop hook
- [ ] Launch posts (see below)

## Post-launch candidates (demand-driven, not speculative)
- Codex / Cursor adapters via `SourceAdapter`
- Subagent transcript stitching
- `--llm` opt-in claim extraction
- Historical trends (`wdyd stats`): verification rate over time per project

## Launch plan
- **Timing:** ride an active "agents lie / who verifies AI code" news moment — a Karpathy-style viral complaint, a model release with agentic claims, or an agent-security incident. The discourse recurs monthly; have M3 ready and wait days, not weeks.
- **Show HN title shape:** "Show HN: Wdyd – ask your coding agent 'what did you do?' and check its answers" (concrete, slightly accusatory, names the moment).
- **Assets:** report screenshot as README hero; 30s GIF: session ends → `wdyd` → red UNVERIFIED line.
- **Seeding:** the PR-markdown flow is the organic loop; personally use it in public PRs from day one.
- **What not to do:** no star-exchange/purchased stars (GitHub purges; taints the account). No fake benchmarks. The tool's whole brand is evidence over narration.

## Success metrics
- Week 1: front-page HN or 500+ stars from launch spike
- Month 1: first community-contributed fixture (proves the contribution loop)
- Month 3: someone else's blog/tweet showing a `wdyd` report catching a real agent lie (proves the screenshot loop)
