//! Claims extractor + verifier. Two halves that never learn each other's job:
//! the extractor pattern-matches `AssistantText` into claims and knows nothing about
//! truth; the verifier matches claims against evidence and knows nothing about regexes.
//! See `docs/architecture.md` §3. Implemented in M2.
