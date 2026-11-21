---
title: Remove the unreachable CodeReads::Scan path and share the lazy code-facts helper of validate and query
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: one way to scan listed code lazily and to merge its diagnostics.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: behavior. `Inputs::generation` (`crates/mara/src/inputs.rs`) maps `CodeReads::Scan` to `CodeReads::List` before `generation_for` and scans afterwards with `scan_listed`, so the `Scan` arms in `generation_for` and the inline-scan path of `Walk` and `Code` are never reached (the reviewer of mara-agdnwh noted the same). `crates/mara/src/cli/validate.rs` (lines 147-160) and `cli/query.rs` (lines 313-336) each build the lazy `scan_listed` to `CodeLinks` to `CodeFacts` callback and deduplicate diagnostics with `contains` in a loop; they differ: `validate.rs` also merges `layer.unread`, `query.rs` does not. Decide which is right.

- [ ] The dead path is gone; one helper serves both, with sort and dedup
- [ ] The difference in merged diagnostics between `project validate` and the query operations is intended or removed; say which in DES-CODE-READERS
