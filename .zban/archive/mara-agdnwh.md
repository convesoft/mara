---
title: A warm code scan in a session copies nothing it does not change and forgets deleted files
status: done
type: bug
parent: mara-779tms
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T15:20:35Z
---

Goal: a daemon request that scans code costs work in proportion to what changed, and a session's memory does not keep code files that no longer exist.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. The code is `Inputs::generation` with `CodeReads::Scan` on a session's view, `Inputs::scan_listed` and `give_back` in `crates/mara/src/inputs.rs` (lines 485-489 for the give-back), as mara-xqcnqw and mara-hk2arx left them. Items: DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), DES-INPUTS-AND-KEYS (docs/engine.mara.md), DES-CODE-READERS and VER-CODE-WORK-COUNTERS (docs/code/), DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- The request adopts the session's `Arc<State>` and still holds it when `give_back` calls `Arc::make_mut(&mut shared.state)`. With two owners, `make_mut` copies the whole `State` (directories, ignore files, parsed documents, programs and the code map) on every code scan a session serves.
- `give_back` calls `tracked.still_as_read(&root.join(path))`, one more `symlink_metadata`, for every code file of the project on every warm scan, also for files the scan kept unchanged.
- `give_back` extends `shared.state.code` with the current files and never removes a path that is no longer among the generation's code paths, so a deleted or renamed code file stays in the session with its outline and contents until the session is dropped.

Done when the three hold and mara-xqcnqw's guarantee stays: a scan that a change overtook is not kept (`inputs::tests::a_scan_that_a_change_overtook_is_not_kept`).

- [x] A warm code scan does not copy `State`
- [x] `give_back` checks only the files the scan read again
- [x] A code file that was deleted or renamed leaves the session's code map; a test shows it
- [x] The overtaken-scan test and the never-stale tests for code (`crates/mara/tests/daemon/fresh.rs`) pass unchanged

## Summary of Changes

- **No `State` copy.** `crates/mara/src/inputs.rs` splits `State`:
  - The inventory (directories, `.gitignore` files, documents, programs) is `Inventory`, shared with views as `Arc<Inventory>`.
  - The record of scanned code files is `Arc<CodeScans>`.
  - The contents kept by `read_code` and the scans kept by `scan_code` are plain maps of the session, which a view never shares.

  A scan, `read_code` and `scan_code` change only the code maps. `scan_listed` lets go of the view's record before `give_back`. `generation` lets go of what the view took from an earlier call of the request (`project()`) before the session derives. A warm scan therefore copies no inventory, and the code record is copied only when a concurrent view holds it while it changes.
- **Only re-read files go back.** `scan` returns `Scanned`: every file it read or kept, plus the paths it read itself (those not the same check, key and scan `Arc` as in the record it started from). `give_back` offers only those paths, skips one the session already holds as it is, and calls `still_as_read` only for the rest. A result the scan only reused is never given back, so an older scan cannot bring back a file that a newer listing dropped. `give_back` then removes from the session's record only the paths of the record the scan started from that the scan did not find (`Scanned::gone`), so a file that a newer scan added after the older one listed stays. A scan that read nothing and found no file gone changes nothing.
- **`read_code` stores only what it read.** It keeps contents only when it read the file (`Tracked::same_check` against the record it started from), so a reused record never re-enters a session that forgot it.
- **Forgetting.** `Inputs::forget_code` drops a path from the contents and grammar-level scans that `get` kept. It runs in every derivation that lists the code files, and after `code_file`'s listing walk (`get`). That walk now drops paths that are documents or programs too, by the same test `code_file` applies to the requested path. The record of scanned files is left to the next scan, which first reuses by key the scan of a file that was only renamed, then drops those of its starting record that it did not find.
- **`Walk` and `Code`.** The `Walk` keeps the code files it reads (`Code::previous`/`next`) apart from the inventory. `Code::new` builds its by-key scan cache only for a scan.
- **Unit tests** in `inputs::tests`:
  - `a_warm_scan_copies_nothing_it_does_not_change`: after a scan that read nothing, the session's inventory and code record are the same allocations and no work is counted. After a scan that read one file again, the inventory is the same allocation and the unchanged file keeps its scan.
  - `a_code_file_that_left_is_forgotten`: a rename, a delete and a new `.gitignore` entry. The listing forgets `get`'s contents and scans, and the scan reads and hashes the renamed file once with 0 code scans, then drops the old paths. Then a rename together with a document include that makes a kept `.mara.md` code file a document, followed by `get` of the new path: `get`'s maps lose both, and the next scan has 0 code scans.
  - `an_older_scan_does_not_bring_back_a_file_a_newer_one_dropped`, and its mirror `an_older_scan_keeps_a_file_a_newer_one_added`: an older scan that gives back a re-read file after a newer scan added `src/added.rs` leaves that file in the record. The mirror fails against the pruning of `c7f0d2b`.
  - `a_scan_that_a_change_overtook_is_not_kept` is unchanged and passes.
- **Warm row test:** `performance` `warm::an_added_removed_or_renamed_code_file_is_scanned_only_when_its_contents_are_new` asserts the code row of VER-CODE-WORK-COUNTERS at 1x against a daemon:
  - added: 1 read, 1 hash, 1 code scan;
  - removed: 0 reads, 0 code scans;
  - renamed with unchanged contents: 1 read, 1 hash, 0 code scans;
  - 0 parses in all three.
- **Docs.** VER-CODE-WORK-COUNTERS: the row "A code file added; one removed" now has the rename case. DES-DAEMON-SESSIONS records the give-back rule, its comparison with the session's record, and the forgetting rule.
- **Counts.** No other expected count changes. The warm rows count no stat, so the `symlink_metadata` saved per unchanged code file per warm scan does not show in them.

Checks at the head that contains `2fa2c1b`, all exit 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`
- `cargo test --workspace --locked`
- `mara --no-daemon project validate`: 0 errors, plus the existing `requirement-verified` warning on docs/distribution.mara.md
- `cargo test --package mara --test performance --release --locked` under `berth --exclusive`
- `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`
- `zban check`

No new crash records.

## Decisions made

- `get` forgets its contents and grammar-level scan of a code file when a listing of the code files no longer finds it as one: deleted, renamed, ignored, no longer matched, or now a document or a program. The listing may be any request's that lists code, or `get`'s own. Recorded in DES-DAEMON-SESSIONS. Rejected: forgetting only in a scan's give-back, which would keep paths that only `get` read until the session is dropped.
- The session's record of scanned files is pruned by the next scan, after it has reused scans by key, not by a listing, so a renamed unchanged file is not scanned again. Recorded in DES-DAEMON-SESSIONS. Rejected: pruning at the listing, which lost the renamed file's scan (review of `bd0991a`).
- `give_back` judges "read again" against the record the scan started from. It compares with the session's record at the moment it gives back only to skip a result the session already holds as it is. A file the scan read itself comes back even when a newer listing dropped it meanwhile, until the next scan: one record and one stat, no stale bytes, since each request checks membership and stats. Recorded in DES-DAEMON-SESSIONS. Rejected: generation counters for listings (orchestrator).
- Follow-up mara-cytrb4 (backlog): the write hand-over and `get`'s listing still copy inventory maps.
