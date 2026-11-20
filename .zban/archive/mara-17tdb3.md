---
title: "0.4.0-alpha.3: project rules with Soufflé"
status: done
type: epic
blocked_by:
  - mara-wamxz3
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-20T15:38:02Z
---

Slice outcome: projects write invariants, queries, gates and views as Soufflé programs, and the engineering template ships its own.

Done when validation of this repository evaluates the engineering template's invariants.

Started on 2026-11-16 while the last two alpha.2 issues (the MCP server and the agent guide) were still in work, by the orchestrator's decision under the maintainer's rule of that day: the first alpha.3 issues build nothing those two touch, and its Verification items (mara-wamxz3) are integrated. The alpha.3 tag still follows the alpha.2 tag.

## Summary of Changes

Closed by the orchestrator on 2026-11-20. Every child is closed (mara-stpjjc and mara-x0qc0q cancelled, the rest done).

- Done when, checked by the orchestrator: `cargo run --package mara --locked --quiet -- --no-daemon project validate` at `6979f4d` (the tree of `f1ee4b4`) evaluates the engineering template's invariants on this repository and reports `docs/distribution.mara.md:7: warning[requirement-verified]`, a diagnostic of the template's `requirement-verified` invariant; result `valid: 0 errors, 1 warning`.
- What the slice delivers: Soufflé built by `scripts/build-souffle.sh`, the fact bridge, the admitted program language, program evaluation with invariants in validation, `query list | execute | view | gate | report`, the engineering template's invariants, queries, gates and views, evaluation limits that leave no crash record, and the Linux sandbox around Soufflé (mara-ptq6xc, from the release slice).
- The workers' `## Decisions made` lists and the orchestrator's decisions are in mara-46j65e.
- The tag `v0.4.0-alpha.3` goes on the version-bump commit that follows.
