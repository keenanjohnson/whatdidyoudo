# Architecture

Core principle: **separate what the agent *said* from what the agent *did*, and only compare the two at the end.**

```
┌────────────┐   ┌────────────┐   ┌──────────────────┐   ┌────────────┐
│ Discovery  │──▶│ Ingestion  │──▶│    Analyzers     │──▶│   Report   │
│ find the   │   │ JSONL →    │   │ blast radius,    │   │ terminal / │
│ session    │   │ Event      │   │ commands, claims │   │ md / json  │
└────────────┘   │ timeline   │   │ verification     │   └────────────┘
                 └────────────┘   └────────▲─────────┘
                                           │
                                  ┌────────┴─────────┐
                                  │ Evidence provider │
                                  │  git + filesystem │
                                  └──────────────────┘
```

## 1. Discovery

Resolves which session(s) to audit. Default: encode the cwd the way Claude Code encodes it for `~/.claude/projects/<encoded-cwd>/`, take the most recently modified JSONL. Flags: `--session <id>`, `--last N`, `--since <date>`, `--all-today`. Filters noise (warmup sessions, `/clear`-only transcripts). Pure path logic — no parsing.

## 2. Ingestion — the schema firewall

The only code that knows the raw Claude Code JSONL format. Streams line-by-line (`BufReader`), deserializes permissively (`#[serde(default)]`, unknown fields ignored, `serde_json::Value` fallback), emits the normalized timeline:

```rust
enum Event {
    UserMessage   { ts: Timestamp, text: String },
    AssistantText { ts: Timestamp, text: String },          // claims live here
    ToolCall      { ts: Timestamp, id: CallId, tool: String, input: ToolInput },
    ToolResult    { ts: Timestamp, call_id: CallId, outcome: ToolOutcome, output: String },
    Unknown       { ts: Option<Timestamp>, raw: serde_json::Value },  // never drop, never crash
}
```

**Schema notes confirmed against real transcripts (2026-07):**

- Each JSONL line has a top-level `type`. Only `user` and `assistant` carry auditable content; `system` / `mode` / `ai-title` / `attachment` / `file-history-snapshot` / `queue-operation` / `last-prompt` are metadata → `Unknown` (noise-filtered in discovery, not ingestion).
- `message.content` is either a **string** (real user prompt) or an **array of blocks** (`thinking` / `text` / `tool_use` for assistant; `tool_result` for user). One line can yield several events. `thinking` blocks are dropped — not auditable.
- `tool_use.id` ↔ `tool_result.tool_use_id` is the `CallId` pairing.
- **There is no numeric exit code in the transcript.** A Bash result's structured `toolUseResult` is `{ interrupted, isImage, noOutputExpected, stderr, stdout }`. Success/failure is *derived* from the `tool_result` block's `is_error: bool` plus `interrupted`, so `ToolResult` carries a `ToolOutcome`, not a synthesized `Option<i32>`:

  ```rust
  enum ToolOutcome { Ok, Failed, Interrupted, Unknown }
  ```

- The top-level `toolUseResult` object is richer than the block content and is the preferred evidence source: Edit/Write results carry `{ filePath, oldString, newString, structuredPatch, userModified, originalFile }` (real edited path + a "did the human change it" flag); subagent results carry `{ agentId, agentType, … }` (edge case #2).
- `input` is normalized to `ToolInput` (a `BTreeMap<String,String>` of the tool call's fields) so no `serde_json::Value` crosses the adapter boundary.

Behind a `SourceAdapter` trait:

```rust
trait SourceAdapter {
    fn detect(path: &Path) -> bool;
    fn parse(reader: impl BufRead) -> impl Iterator<Item = Event>;
}
```

`ClaudeCodeAdapter` first; `CodexAdapter` / `CursorAdapter` are future adapters that touch nothing downstream. Malformed lines degrade to `Unknown` per-line, never abort per-file.

## 3. Analyzers — pure passes over `&[Event]`

Each is a pure function `&[Event] → AnalyzerOutput` (unit-testable against fixture timelines, no I/O):

- **Blast radius** — paths from Write/Edit/Create tool calls; cross-checked against git evidence; in-scope/out-of-scope annotation relative to the task (heuristic: paths mentioned in the initial user message + their directories).
- **Commands** — classify Bash calls (test / build / package-install / git / other) via pattern tables; pair with exit codes from matched `ToolResult`s.
- **Dependencies** — `npm install` / `cargo add` / `pip install` / etc. patterns + lockfile edits; flag installs never mentioned in user messages.
- **Hygiene** — TODOs, `dbg!`/`console.log`/`print(` droppings in the final diff.
- **Claims extractor** — pattern-matches `AssistantText` against a small taxonomy → `Claim { kind, ts, quote_span }`. Knows nothing about truth.

```rust
enum ClaimKind { TestsPass, BuildSucceeds, FileCreated(PathBuf), BugFixed, Committed }
```

- **Claims verifier** — for each claim, interrogate evidence *before the claim's timestamp*:
  - `TestsPass` → test-category command with exit 0 (and pass-summary output)?
  - `BuildSucceeds` → build command exit 0?
  - `FileCreated(p)` → matching Write/Edit call, and file exists on disk now?
  - `Committed` → git log confirms in session window?

```rust
enum Verdict { Verified(EvidenceRef), Unverified, Contradicted(EvidenceRef) }
```

Extraction and verification are deliberately separate: extraction failures are regex gaps (patch with fixtures); verification failures are logic bugs. The split also leaves a seam for an optional `--llm` extractor later (v2, opt-in, never default) without touching verification.

## 4. Evidence provider

The verifier's only window onto reality outside the transcript. Trait-based for mocking:

```rust
trait Evidence {
    fn diff_stat(&self, since: &GitRef) -> Result<DiffStat>;
    fn commits_since(&self, ts: Timestamp) -> Result<Vec<Commit>>;
    fn file_exists(&self, p: &Path) -> bool;
}
```

Git via `std::process::Command` (`git diff --stat`, `git log`) — not libgit2; more robust across odd repo states. Mid-session commits handled by walking `git log` from a pre-session ref rather than diffing only the working tree. **This trait plus transcript reading is the tool's entire I/O surface. No network, ever.**

## 5. Report engine

All analyzers feed one struct:

```rust
struct AuditReport {
    session: SessionMeta,
    blast_radius: BlastRadius,
    claims: Vec<(Claim, Verdict)>,
    dependencies: Vec<DepChange>,
    hygiene: Vec<Finding>,
    trust_summary: TrustSummary,   // "2 verified · 1 UNVERIFIED · scope 64%"
}
```

Renderers consume only `AuditReport`: terminal (`comfy-table` + `owo-colors`, screenshot-friendly), `--md` (paste into PRs), `--json` (scripting/CI). `--check` maps the trust summary to an exit code.

## Cross-cutting decisions

- **Timestamps are the spine.** Claim-before-evidence ordering, session windows, git-log windowing. Normalize to UTC instants at ingestion; never compare strings.
- **Fixtures are the moat.** `fixtures/` holds anonymized JSONL from different Claude Code versions, each paired with an `insta` snapshot of the expected `AuditReport`. Format churn shows up as a failing snapshot, not a user bug report. Community-contributed fixtures are the cheapest external contribution — encourage them.
- **No async.** Sequential file processing; tokio is dependency weight.
- **lib + thin bin.** Logic in library modules; `main.rs` is arg-parsing and wiring.

## Crate map

`clap` (derive) · `serde` / `serde_json` · `walkdir` · `regex` · `comfy-table` · `owo-colors` · `anyhow` (bin) / `thiserror` (lib) · `insta` (dev) · `cargo-dist` (release: binaries, Homebrew tap, install script).

## Known edge cases (handle from day one — these will be the first issues filed)

1. Agent committed mid-session → walk `git log` from pre-session ref.
2. Subagent transcripts → separate JSONL entries/files belonging to one logical session.
3. Warmup / `/clear`-only sessions → filter in discovery.
4. Multi-session tasks → `--since` accepts a date or git ref.
5. Very large sessions → streaming only; never whole-file loads.
6. Non-UTF8 / truncated lines → `Unknown` event, keep going.
