---
title: get of a document or an item lists no code
status: in-progress
type: bug
parent: mara-779tms
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: `get` of a document path, of a line in a document, or of an address that is no code file reads no code: it walks no code directory, reads no `.gitignore` under a code root, adds no code directory to the session's watches and reports no diagnostic of the code tree.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. DES-CODE-READERS (docs/code/index.mara.md) says which operations read code; `get` of an item or a document is not one. In `crates/mara/src/cli/read.rs`, `code_path` returns a path for every address that is neither an ID nor a MID, so also for `docs/a.mara.md` and `docs/a.mara.md:10`. When `Inputs::code_file` says the path is no code file, the fallback calls `inputs.generation(context.now, context.work, CodeReads::List)` (near line 335), which lists all code files; before mara-b76x4g every `get` did. The orchestrator read the code and confirms the call. Items: DES-CODE-READERS, VER-CODE-WORK-COUNTERS (docs/code/verification.mara.md), REQ-WORK-FOLLOWS-CHANGE (docs/performance.mara.md).

Done when a work-counter test shows that `get` of a document path in a project with a code language reads and hashes what `get` of the same item by ID does, and returns no diagnostic of a code file.

- [ ] The fallback of `get` for an address that is no code file reads the documents only (`CodeReads::None`)
- [ ] A test with a code language configured and a problem in the code tree: `get` of a document path counts no code read and returns no code diagnostic, cold and through a daemon
