---
title: Review the four engineering views that were left out
status: done
type: task
parent: mara-qzp1gd
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-16T10:42:59Z
---

Goal: decide before the release whether the four views that the first `engineering` template leaves out bring enough value to ship.

Context: DES-TEMPLATE-VIEWS in docs/template-readiness.mara.md ships three of the seven designed views: delivery readiness, requirement verification and risk treatment. The maintainer asked on 2026-11-14 to come back to the other four before the release. Their design is in `sources/2026-10-08-space-export/Engineering process/Approach/Draft views and traceability matrices.md` under `~/Work/local/workspace/projects/mara-rewrite-design/engineering-template-inputs/` on the maintainer's machine.

- [x] Outcome coverage and validation: ship, defer or drop: deferred
- [x] Engineering basis and rationale: ship, defer or drop: deferred
- [x] Knowledge validity: ship, defer or drop: deferred
- [x] Change impact: ship, defer or drop; it needs a traversal from the named items that nothing else uses: deferred
- [x] DES-TEMPLATE-VIEWS updated with the outcome: by mara-tx0fcf, in its branch

## Summary of Changes

Decided provisionally by the orchestrator on 2026-11-16, under the maintainer's rule of that day that it decides on the maintainer's behalf. No code or docs changed here.

All four views are deferred to after 0.4.0 and collected in backlog issue mara-7h4z9f. Reasons: the first template is useful with the three views DES-TEMPLATE-VIEWS ships; a view can be added in a later release without breaking a project, so this is the option easiest to change; change impact needs a traversal that nothing else uses; and the tool becomes usable sooner. Which views users get is a capability they will see, so this is the maintainer's to confirm: it is provisional and listed in mara-46j65e. Undo: pull mara-7h4z9f, or a single view of it, into a slice. mara-tx0fcf adds the sentence to DES-TEMPLATE-VIEWS.
