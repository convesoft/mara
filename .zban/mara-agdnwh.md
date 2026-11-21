---
title: A warm code scan in a session copies nothing it does not change and forgets deleted files
status: in-progress
type: bug
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
---

Goal: a daemon request that scans code costs work in proportion to what changed, and a session's memory does not keep code files that no longer exist.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. The code is `Inputs::generation` with `CodeReads::Scan` on a session's view, `Inputs::scan_listed` and `give_back` in `crates/mara/src/inputs.rs` (lines 485-489 for the give-back), as mara-xqcnqw and mara-hk2arx left them. Items: DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), DES-INPUTS-AND-KEYS (docs/engine.mara.md), DES-CODE-READERS and VER-CODE-WORK-COUNTERS (docs/code/), DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- The request adopts the session's `Arc<State>` and still holds it when `give_back` calls `Arc::make_mut(&mut shared.state)`. With two owners, `make_mut` copies the whole `State` (directories, ignore files, parsed documents, programs and the code map) on every code scan a session serves.
- `give_back` calls `tracked.still_as_read(&root.join(path))`, one more `symlink_metadata`, for every code file of the project on every warm scan, also for files the scan kept unchanged.
- `give_back` extends `shared.state.code` with the current files and never removes a path that is no longer among the generation's code paths, so a deleted or renamed code file stays in the session with its outline and contents until the session is dropped.

Done when the three hold and mara-xqcnqw's guarantee stays: a scan that a change overtook is not kept (`inputs::tests::a_scan_that_a_change_overtook_is_not_kept`).

- [ ] A warm code scan does not copy `State`
- [ ] `give_back` checks only the files the scan read again
- [ ] A code file that was deleted or renamed leaves the session's code map; a test shows it
- [ ] The overtaken-scan test and the never-stale tests for code (`crates/mara/tests/daemon/fresh.rs`) pass unchanged
