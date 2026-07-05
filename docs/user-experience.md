# User experience

## Persona & the moment

A developer running Claude Code several times a day, increasingly kicking off longer autonomous sessions. They return to "Done! All tests pass, I've fixed the bug." They've been burned. Today's ritual: `git diff --stat`, squint at 14 changed files when 3 were expected, scroll terminal history hunting for whether a test command actually ran, give up halfway.

The product exists for **the 30 seconds between "agent says done" and "do I trust this enough to commit."**

## Design constraints (non-negotiable)

1. **`wdyd` with no arguments must do the right thing.** The user is mildly annoyed and pre-coffee. Zero decisions on the golden path.
2. **Value on first run, against history that already exists.** No wrapper, no proxy, no init, no account. Every competitor that demands workflow change before payoff filters out ~95% of casual evaluators. Instant retroactive gratification is the core growth mechanic.
3. **Fast enough to be muscle memory** (<1s typical) — the mental model is `git status` for agent sessions.
4. **The terminal report must look good in a screenshot** and the trust summary must read as a headline. "My agent lied to me" screenshots are the free marketing channel.

## Installation tiers

```bash
cargo install whatdidyoudo                     # day one (Rust audience)
brew install whatdidyoudo                      # week one — the one that matters for growth
curl -fsSL https://<domain>/install.sh | sh    # shell installer (cargo-dist generates)
# prebuilt binaries: GitHub Releases (macOS / Linux / Windows)
# later, optional: npx whatdidyoudo (unscoped npm name is free)
```

`cargo-dist` automates releases, the Homebrew tap, and the install script.

## Flows

### Golden path (first run & daily habit)
Just finished a session, in the project directory:

```
$ wdyd
```

Finds the matching Claude project folder, grabs the most recent session, prints the one-screen report (blast radius / claims / dependencies / left-behind / trust summary). The red `NO TEST COMMAND EVER RAN` line is the conversion moment — concrete, checkable, confirms an existing suspicion.

Variants: `wdyd --last 3` after stacked morning sessions; `wdyd --session <id>`.

### PR flow (the growth loop)
`wdyd --md | pbcopy` → paste into the PR description. Teammates see the audit block and ask "what's that tool?" This is why the markdown renderer is v1, not a nice-to-have.

### Enforcement flow (power-user hook)
`wdyd --check` exits non-zero on unverified/contradicted claims or scope compliance below threshold. Composes into:
- git pre-commit hooks (bad session physically blocks the commit)
- CI jobs
- Claude Code Stop hooks (audit fires automatically the instant a session ends)

Ship the exit code + README recipes; the community wires the rest.

### Forensic flow
Something broke Thursday; five sessions happened this week.

```
$ wdyd --since monday
```

Lists sessions with one-line trust summaries; drill into the suspicious one. Lower frequency, high memorability — it attaches to incidents.

## Anti-goals

- No live session monitoring (that's Grass/Nimbalyst territory and requires workflow adoption).
- No cost/token reporting (saturated; 11k-star incumbent).
- No transcript browsing UI (saturated; viewers exist).
- No default LLM calls (offline-by-default is identity; `--llm` extraction is a possible opt-in v2).
