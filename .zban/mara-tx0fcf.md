---
title: Engineering template programs
status: in-progress
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
  - mara-gfb6n6
created_at: 2026-11-13T22:10:06Z
---

Goal: the `engineering` template ships the invariants, queries, gates and views that make its vocabulary useful.

Context: docs/template.mara.md (REQ-TEMPLATE-ACCEPTANCE-RULES, DES-TEMPLATE-ACCEPTANCE, DES-TEMPLATE-RELATION-LIFECYCLE, DES-TEMPLATE-RISK-LEVEL); docs/template-readiness.mara.md (REQ-TEMPLATE-GATES-PROVE-READINESS, DES-TEMPLATE-QUERIES, DES-TEMPLATE-INPUTS, DES-TEMPLATE-GATES, DES-TEMPLATE-VIEWS); VER-TEMPLATE-CASES in docs/verification.mara.md.

Design input, on the maintainer's machine: `~/Work/local/workspace/projects/mara-rewrite-design/engineering-template-inputs/README.md` and the Space export beside it. The previous encoding is in `git show d7f0e73:templates/` and `git show d7f0e73:docs/engineering-template.mara.md`; its helper queries, view shims and separate `required_validation` gate were limits of the old engine and are not ported.

- [x] Decide with the maintainer which invariants, queries, gates and views ship; record them as items
- [ ] Programs for the acceptance and lifecycle invariants, the queries, the three gates, the three views and the risk matrix, written in the admitted language (DES-PROGRAM-PROFILE) and installed by `project init`; the programs row of VER-PROJECT-INIT passes
- [ ] The cases of VER-TEMPLATE-CASES that need no code links pass. Without code links, `verified` and `release-ready` are tested for what they can show: a known failure fails and everything else is unavailable; mara-yhxq5x makes the remaining cases pass
- [ ] This repository adopts the invariants, with `requirement-verified` as a warning until the last slice

Decided provisionally by the orchestrator on 2026-11-16 (from mara-b60px3, under the maintainer's rule of that day; listed in mara-46j65e): the four views that the first template leaves out (outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact) are deferred to after 0.4.0; backlog issue mara-7h4z9f.

- [ ] DES-TEMPLATE-VIEWS says in one sentence that the other four designed views are not in the first release
