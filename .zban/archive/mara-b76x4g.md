---
title: get of a code path parses no document
status: done
type: task
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T13:06:04Z
---

Goal: `mara get <code-path>` and `get <code-path>:<line>` do not parse the project's documents or build a snapshot to answer about a code file.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. `crates/mara/src/cli/read.rs:321`: `get` calls `inputs.generation(context.now, context.work, CodeReads::List)`, which walks the project, parses every document and builds the snapshot, and then drops the snapshot as soon as the address is found among `generation.code_paths`. Items: DES-CODE-READERS and VER-CODE-WORK-COUNTERS (docs/code/), the `get` row of the work-counter tests (docs/verification.mara.md).

Done when a `get` of a code path reports 0 document parses and 0 snapshot builds, asserted by a work-counter test, and VER-CODE-WORK-COUNTERS' `get` row says so.

- [x] Decide whether the address is a code file first (the listing, or the configured include patterns) and build the snapshot only when it is not
- [x] The `get` row of VER-CODE-WORK-COUNTERS and its test require 0 parses and 0 snapshot builds for a code path, at the basic and at the grammar level
- [x] An address that is no code file behaves as before

## Summary of Changes

- `crates/mara/src/inputs.rs`: new `Inputs::code_file(path, now, work)` (addition only). It tests the path against the include patterns and leaves out documents, programs and temporary files. When the path can be a code file, it runs the existing `Walk` with `CodeReads::List` on a copy of the configuration that has no document or program patterns. The copy starts from the last check's directories and `.gitignore` files and gives nothing back. It returns the language and the listing's diagnostics, reads no document or program, builds no snapshot and changes no state of the inputs or of a session.
- `crates/mara/src/cli/read.rs`: `get` asks `code_file` first. For a code file it reads, scans and returns the file. Otherwise it takes the earlier path: a `CodeReads::List` generation, then `read::get` on the documents.
- `docs/code/index.mara.md` (DES-CODE-READERS) records the decision below. `docs/code/verification.mara.md` (VER-CODE-WORK-COUNTERS) now has a cold `get` row with 0 parses and 0 snapshot builds at either level, and a new warm `get` row.
- Tests: the new `crates/mara/tests/performance/code_get.rs` checks a line and a whole file at the basic and the grammar level, both cold and warm (through a daemon the test starts, including after a code file changes on disk). It also checks that the session keeps its snapshot, and that `get` of a document's path costs the same as `get` of its item's ID. In `cli.rs` and `work_counters.rs`, the cold case `get <code>:2` now uses `Reads::Code`: 3 reads and 3 hashes (the configuration, the schema and the root `.gitignore`), plus the code file.
- Checks after merging `d9e4bff`: fmt, clippy, `cargo test --workspace --no-fail-fast`, `project validate` (1 warning that was already there), the release performance test under `berth --exclusive`, the wasm32 build and `zban check` all pass. No new coredumps.

## Decisions made

- `get` of a code location tests the configured patterns before it lists anything, and then lists only where a code include pattern can match (DES-CODE-READERS). It reads no document or program and builds no snapshot.
- Its diagnostics are those of its own listing and of the scan, not the inventory's diagnostics of the documents. DES-READ-RESULTS asks for those only on a read of documents.
- A path that matches a code pattern but is not listed as code (ignored, missing, or below another project's root) falls back to the earlier document path. It reads each `.gitignore` that both walks visit once more than before, and fails as before.

Rejected: a check that visits only the path's ancestors. It would repeat `Walk`'s `.gitignore` and nested-root logic, and DES-CODE-READERS says `get` lists the code files.

## Review round

- DES-PAGING (docs/retrieval/index.mara.md) now states what keys the cursor of a `get` of a code location: the hash of the code file's contents, or the key of its scan when its language's grammar scanned it, and the code, path, line and message of each diagnostic it returns. It links to DES-CODE-READERS. A cursor whose key no longer matches fails with `stale-cursor` (`ReadError::StaleCursor`, `crates/mara-engine/src/read/node.rs`), as "A cursor of another result is stale" says. A cursor issued before this change no longer continues only when the documents had inventory diagnostics then.
- `code_get.rs` builds on every platform. Only its warm test, which finds daemons in `/proc`, is gated to Linux.
