---
title: get of a document or an item lists no code
status: done
type: bug
parent: mara-779tms
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
closed_at: 2026-11-21T17:39:15Z
---

Goal: `get` of a document path, of a line in a document, or of an address that is no code file reads no code: it walks no code directory, reads no `.gitignore` under a code root, adds no code directory to the session's watches and reports no diagnostic of the code tree.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. DES-CODE-READERS (docs/code/index.mara.md) says which operations read code; `get` of an item or a document is not one. In `crates/mara/src/cli/read.rs`, `code_path` returns a path for every address that is neither an ID nor a MID, so also for `docs/a.mara.md` and `docs/a.mara.md:10`. When `Inputs::code_file` says the path is no code file, the fallback calls `inputs.generation(context.now, context.work, CodeReads::List)` (near line 335), which lists all code files; before mara-b76x4g every `get` did. The orchestrator read the code and confirms the call. Items: DES-CODE-READERS, VER-CODE-WORK-COUNTERS (docs/code/verification.mara.md), REQ-WORK-FOLLOWS-CHANGE (docs/performance.mara.md).

Done when a work-counter test shows that `get` of a document path in a project with a code language reads and hashes what `get` of the same item by ID does, and returns no diagnostic of a code file.

- [x] The fallback of `get` for an address that is no code file reads the documents only (`CodeReads::None`)
- [x] A test with a code language configured and a problem in the code tree: `get` of a document path counts no code read and returns no code diagnostic, cold and through a daemon

## Summary of Changes

- `crates/mara/src/cli/read.rs`: when `Inputs::code_file` finds no code file at the address, `get` reads the documents with `CodeReads::None` (through `Inputs::snapshot`, the same read as `get` by ID, now one shared branch) instead of `CodeReads::List`. So it no longer walks the code directories, reads the `.gitignore` files under the code roots, adds code directories to a session's watches or reports the code tree's diagnostics. `Inputs::code_file` was left alone: it returns before walking anything for a document's path and for a path that no code include pattern matches.
- `crates/mara/tests/performance/code_get.rs`: two new tests, `cold_get_of_a_document_reads_no_code` and `warm_get_of_a_document_reads_no_code`. They run at the basic level and with the Rust grammar pinned and cached, on a corpus with a `.gitignore` in a code directory and a `.gitignore` that is a directory below it (`project validate` reports it). For `get` of a document's path, of `<path>:1` and of `notes.txt`:
  - cold, the work counts equal those of `get` by ID;
  - warm, through a daemon the test starts, every count is 0 and `served` from `daemon status` rises by one per request;
  - neither run reports a diagnostic.
  
  Before the fix, the cold run counted 31 reads and hashes against 30, and the warm run counted 1 read and 1 hash. The test that ran on the old code is moved into these tests.
- `docs/code/index.mara.md` DES-CODE-READERS and `docs/code/verification.mara.md` VER-CODE-WORK-COUNTERS: see Decisions made.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `mara --no-daemon project validate` (0 errors, the existing warning on docs/distribution.mara.md), `cargo test --package mara --test performance --release --locked` (30 passed, 2 ignored), the wasm32 build of `mara-engine` and `zban check`. All exit 0.

## Decisions made

- DES-CODE-READERS: a document's path, a line in a document and a path that no code include pattern matches are read as `get` of an item is. Nothing is listed, watched or read for code alone, and no diagnostic of code is reported. A directory that holds documents and code is still listed and watched for its documents. A path that a code include pattern matches and that is no code file is read as a document after that listing.
- VER-CODE-WORK-COUNTERS gains a cold row and a warm row. The cold row: `get` of a document's path, of a line in it, or of a path that no code include pattern matches, with a `.gitignore` and a problem in the code tree, at the basic and at the grammar level, counts what `get` of the item by ID counts. The warm row: the same `get` counts 0 at either level, the session holding the snapshot and the inputs settled. Neither reports a diagnostic of code.
- Out of scope, left to mara-h4z424: an address that a code include pattern matches but that is no file (such as `src/missing.rs`) still needs a listing to decide that it is no code file. DES-CODE-READERS allows that listing. mara-h4z424 limits it to the path's ancestor directories.
