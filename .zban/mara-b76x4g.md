---
title: get of a code path parses no document
status: todo
type: task
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
---

Goal: `mara get <code-path>` and `get <code-path>:<line>` do not parse the project's documents or build a snapshot to answer about a code file.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. `crates/mara/src/cli/read.rs:321`: `get` calls `inputs.generation(context.now, context.work, CodeReads::List)`, which walks the project, parses every document and builds the snapshot, and then drops the snapshot as soon as the address is found among `generation.code_paths`. Items: DES-CODE-READERS and VER-CODE-WORK-COUNTERS (docs/code/), the `get` row of the work-counter tests (docs/verification.mara.md).

Done when a `get` of a code path reports 0 document parses and 0 snapshot builds, asserted by a work-counter test, and VER-CODE-WORK-COUNTERS' `get` row says so.

- [ ] Decide whether the address is a code file first (the listing, or the configured include patterns) and build the snapshot only when it is not
- [ ] The `get` row of VER-CODE-WORK-COUNTERS and its test require 0 parses and 0 snapshot builds for a code path, at the basic and at the grammar level
- [ ] An address that is no code file behaves as before
