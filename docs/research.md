# Research: demand & competitive landscape

Compiled July 2026. Sources checked via web research; registry availability checked live against crates.io / npm / PyPI / Homebrew / GitHub APIs.

## The problem, in the market's own words

- **People already do this audit manually, and write tutorials about it.** codeongrass.com published a step-by-step post-run audit guide: `git diff HEAD --stat` to map touched files, manual scope-compliance scoring (in-scope vs out-of-scope changes, with an 80% threshold heuristic), and inspection of Claude Code's JSONL tool-call traces at `~/.claude/projects/<encoded-cwd>/<session-id>.jsonl`. Estimated 5–10 minutes per session, recommended after every session. A manual workflow with tutorials is the classic precursor signal for a tool that automates it. (https://codeongrass.com/blog/how-to-audit-ai-agent-post-run-drift/)
- **"Agents lie about their work" is named discourse.** A widely shared dev.to piece describes agents generating completion language regardless of codebase state — writing "tests passing" while the suite has syntax errors, claiming files exist that were never written — and argues transcript parsing alone misses the subtle failures; verification must check what actually happened. (https://dev.to/moonrunnerkc/ai-coding-agents-lie-about-their-work-outcome-based-verification-catches-it-12b4)
- **HN cares.** Thread "When AI writes the software, who verifies it?" (March 2026): commenters describe LLM-generated tests that merely reinforce existing behavior while humans rubber-stamp merges. (https://news.ycombinator.com/item?id=47234917)
- **Best practice explicitly says: don't trust self-reports.** Provenance guidance: "Do not rely on self-attestation alone. If an agent says it ran tests, prefer captured command output, CI logs, or reproducible checks." OWASP agentic guidance calls for structured logs around file access, shell commands, network calls. GitHub's Copilot cloud agent docs point reviewers at session logs and audit events. (https://medium.com/toward-next-ai/ai-code-provenance-workflow-track-what-coding-agents-changed-before-it-ships-02cd387cbba3, https://www.developersdigest.tech/blog/permissions-logs-rollback-ai-coding-agents)
- **Category validation.** The repo encoding Karpathy's viral complaints about agent failure modes (silent wrong assumptions, over-engineering, out-of-scope changes) as a single CLAUDE.md hit ~156k stars. Out-of-scope changes — our "blast radius" — is one of the three named failure modes. (https://www.firecrawl.dev/blog/best-github-repos)

## Competitive landscape

### Viewers / exporters (adjacent, not competing — they show, we judge)
- `simonw/claude-code-transcripts` — JSONL → shareable HTML transcripts. Note: its Claude-Code-for-web commands broke when unofficial APIs changed — evidence of the schema-churn risk.
- `daaain/claude-code-log` — JSONL → HTML/Markdown, TUI, token usage display.
- `raine/claude-history` — fuzzy search over conversation history.
- `jhlee0409/claude-code-history-viewer` — desktop viewer spanning 25 assistants.
- `jazzyalex/agent-sessions` — macOS app, browse/search/resume across Codex, Claude, Cursor, etc.
- `d-kimuson/claude-code-viewer` — web client for session log analysis.

### Cost / usage analyzers (proves the distribution model, different question)
- Leading local JSONL usage-analysis CLI: 11,500+ stars, "offline, zero API calls" positioning.
- Several others: cost-leak detectors, token attribution dashboards (tokburn, claude-token-lens, etc.). All parse the same local JSONL we do. None answer the trust question.

### Heavyweight verification layers (competing on the question, losing on weight)
- **Swarm Orchestrator** — runs agents on isolated branches, cross-references each claim against transcript + filesystem, gates merges on quality checks. Requires adopting the orchestrator.
- **MartinLoop** — governed runs with budget limits and a `--verify` command; MCP server integration. Requires running sessions through it.
- **Enterprise audit logging** — Nylas CLI agent audit logs, MCP gateways (MintMCP et al.) with before/after diffs and SOC 2 framing. Requires infrastructure.
- **Startups** — BuildOrbit ("verifiable execution runtime", pre-revenue), Grass/codeongrass (session monitoring product), Nimbalyst (session management with file-change tracking).

### Closest overlap found
- A "session intelligence" tool in the awesome-claude-code-toolkit list: reads `~/.claude/projects/` JSONL to surface token waste, CLAUDE.md adherence failures, attention-curve degradation. Efficiency/adherence oriented, not claims-vs-evidence trust verdicts. Watch it.

## The open gap

Nothing found that is simultaneously: (a) free/open-source, (b) zero-config and retroactive — works on sessions that already happened with no workflow change, (c) verdict-oriented — a claims-vs-evidence table with verified/unverified/contradicted outcomes, rather than a browsable transcript or a cost report.

## Risks

1. **Platform risk** — Anthropic (or any agent vendor) ships a native `/audit`. True of the whole ecosystem; hasn't stopped the cost analyzers.
2. **Schema churn** — the JSONL is undocumented and changes between versions; already broke tools in this space. Mitigation: permissive parsing, `Unknown` fallback, fixtures-based regression tests, Claude-Code-local-only scope for v1.
3. **Near-neighbor drift** — the session-intelligence tool or a cost analyzer adds a claims table. Mitigation: ship fast, own the "trust report" framing.

## Name availability (checked live, July 2026)

| Ecosystem | `whatdidyoudo` | `wdyd` |
|---|---|---|
| crates.io | free | free |
| npm | free | taken (dormant commit-message CLI, last publish 2022) |
| PyPI | taken (active OSM Flask app) | free |
| Homebrew core | free | free |
| GitHub | 14 tiny repos, max 18 stars | 29 tiny repos, max 4 stars |

Decision: repo + crate `whatdidyoudo`, binary `wdyd` (ripgrep/rg pattern). Also register the free `wdyd` crate as a reservation. Note "wdyd" is texting slang for "what did you do" — reinforces meaning, but generic searches for the short form are polluted; the full name searches clean.
