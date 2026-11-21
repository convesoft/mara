---
title: Link the requirements that no single place implements through design markers
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-cp1skx
created_at: 2026-11-21T21:19:43Z
---

Goal: REQ-NEAR-LINEAR-SCALING and REQ-READS-NEVER-WRITE count as implemented in this repository's own readiness queries, and the scanned files cannot silently lose a new source directory.

Context: mara-cp1skx (commit `b561961`) linked 43 of the 51 Accepted requirements to code with `@mara implements` markers (the convention is in AGENTS.md, under "Mara documents"). These two are built, but every operation carries them out, so no function or type could be marked; the gates `verified` and `release-ready` of the engineering template report them `not_implemented`. The query `implemented` (`.mara/rules/readiness.dl`) also counts a code link to an Accepted design that satisfies the requirement. Mara's include patterns have `**`, `*` and `?` and no exclusion, so `[code]` in `.mara/config.toml` lists the engine's source directories one by one to leave out `crates/mara-engine/src/code/tests.rs`, whose test data reads as markers; a new directory under `crates/mara-engine/src/` is not scanned until a pattern names it. The worker of mara-cp1skx and the orchestrator recommend no product change for exclusion until another project needs it.

- [ ] Mark the code that implements the designs satisfying REQ-NEAR-LINEAR-SCALING and REQ-READS-NEVER-WRITE, where such a design has a place, and say in the summary which of the two then counts as implemented
- [ ] Decide how a new source directory under `crates/mara-engine/src/` is kept from going unscanned (move the test data that reads as markers so that `crates/**/src/**/*.rs` can be the pattern, or a check), without adding exclude patterns to the product

Note from the orchestrator, 2026-11-21 (architecture audit after alpha.5): the auditor's way for the second item: build the marker-shaped literals of `crates/mara-engine/src/code/tests.rs` so that no source line reads as a marker (for example `concat!("@", "mara ")`), or move that test data under `crates/mara-engine/tests/`; `[code]` then needs two patterns, `crates/mara/src/**/*.rs` and `crates/mara-engine/src/**/*.rs`, and no new directory goes unscanned.
