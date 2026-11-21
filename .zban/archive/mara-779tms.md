---
title: "0.4.0-alpha.5: code links"
status: done
type: epic
blocked_by:
  - mara-ttvp9z
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-21T21:20:19Z
---

Slice outcome: `@mara` comment markers link code to items, at file-and-line level everywhere and at declaration level with a grammar.

Done when this repository's code links to the requirements it implements.

Started on 2026-11-16 beside alpha.3 and alpha.4, by the orchestrator's decision under the maintainer's rule of that day: comment markers at the basic level build on neither the daemon nor the rules. The parts that do (mara-hk2arx, mara-9fphtg) are blocked by the issues they need.

## Summary of Changes

Closed by the orchestrator on 2026-11-21, with all fifteen issues of the slice closed.

- "Done when", run by the orchestrator at the tree of `b561961`: `cargo run --package mara --locked --quiet -- --no-daemon query execute implemented --limit 100` lists 94 code links on 43 of the 51 Accepted requirements; `project validate` reports 0 errors and the one known warning (`requirement-verified`, docs/distribution.mara.md); the seven local checks of AGENTS.md pass, with no daemon runtime directory created and no process left.
- The eight requirements without a code link: four are built by later slices (REQ-NODE-LIBRARY, REQ-BROWSER-LOCAL-WORKSPACE, REQ-BROWSER-ACCESS-AUTHORIZED, REQ-RUN-WITHOUT-TOOLCHAIN); two have no single place (backlog mara-006gqt); two are built in Datalog programs, which carry no markers (the maintainer's question mara-pmjt08).
- Integrated in the last run: mara-yk8drm `3203019`, mara-0t6mpc `e2c1706`, mara-7kfxn1 `2933d24`, mara-h4z424 `3805e1e`, mara-9fphtg `254a511`, mara-cp1skx `b561961`.
- The decisions of the slice are in mara-7ztypp: the orchestrator's lines and the `## Decisions made` lists of its issues.
