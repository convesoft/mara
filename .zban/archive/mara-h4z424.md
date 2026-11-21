---
title: get of a code location checks only that file's directories and does not choose kept contents by time
status: done
type: bug
parent: mara-779tms
blocked_by:
  - mara-0t6mpc
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
closed_at: 2026-11-21T18:41:38Z
---

Goal: `get` of a code location in a session costs work for that file and its ancestor directories only, and which kept contents answer it does not depend on a clock.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. `crates/mara/src/inputs.rs` as mara-b76x4g, mara-p27z9t and mara-agdnwh left it. Related backlog: mara-cytrb4 (`code_file` clones the inventory's directory and `.gitignore` maps), mara-2ndg14.

- `Inputs::code_file` runs a full walk over every configured language's directories on every `get` of a code location, reads and hashes the `.gitignore` files under the code roots when the session has not listed code before, reports diagnostics of unrelated code subtrees, and discards what the walk learned.
- `Inputs::read_code` picks between the contents an earlier `get` kept and the contents a scan kept by comparing their check times (`scanned.checked() > got.checked()`). DES-DAEMON-SESSIONS (docs/daemon/index.mara.md) says a session never decides by time: a clock that stands still or steps back gives the wrong one.
- mara-agdnwh already forgets kept contents and scans of files that left; check that nothing of this issue is left there.

Items: DES-CODE-READERS (docs/code/index.mara.md), DES-DAEMON-SESSIONS, DES-INPUTS-AND-KEYS (docs/engine.mara.md), VER-CODE-WORK-COUNTERS (docs/code/verification.mara.md).

- [x] `get` of a code location checks the path's ancestor directories and their `.gitignore` files, not every code directory, and keeps what it learned in the session; its diagnostics are those of that path
- [x] One record of the latest checked contents per code path, or an order that is no clock, decides which kept contents answer
- [x] Work-counter tests: a repeated `get` of a code location in a session that never listed code reads nothing again

## Summary of Changes

- `Inputs::code_file` (`crates/mara/src/inputs.rs`) runs the inventory's walk limited to one path (`Walk::only`). It lists the root and the directories strictly above the path, reads only their `.gitignore` files, and takes the path itself only as a file. A directory named like the path is neither listed nor reported. It reads no other code directory's `.gitignore` and reports no other directory's problem.
- The walk chooses each record where it reaches a directory or `.gitignore`, after the parent admitted it, from the metadata it took there. `Walk::kept` holds the session's `State::ancestors` records beside `previous`, the inventory's, and `held` picks the one a check keeps. No record is stat'd through a symbolic link, and none is stat'd twice. `code_file` passes O(depth) records and clones no inventory map. `read_code` stats the file once, after `code_file` admitted its directories in the same request.
- What `code_file` lists or reads goes into `State::ancestors`. Every walk of the inputs also uses those records and refreshes them for the directories it lists (`refresh_ancestors`). A listing of the code files drops the directories it does not reach. So `get`, `project validate`, `item list`, `get` reads nothing on the last `get`.
- `get` forgets kept contents and scans only for its own path, when that path is no code file, by two removals by key.
- `Inputs::read_code` no longer compares check times: of the contents a `get` kept and those a scan kept, it uses the one a check keeps (same stat, settled), through `Tracked::holds` / `Listing::holds` in `tracked.rs`.
- `get` (`crates/mara/src/cli/read.rs`) attaches the diagnostics of the check on the way to its failure when a path that a code include pattern matches is no code file. This was mara-v1bnwd, now closed.
- Tests, each failing against 2674624 and passing now:
  - `code_get.rs`, cold: a directory named like the path does what a missing path does (32 reads before the fix, 31 now).
  - `code_get.rs`, warm: `get`, `get`, `project validate`, `item list`, `get` reads 0 on the last `get` (1 read before the fix).
  - `tests/code/markers.rs`: `get src/lib.rs` under `[z-a].rs` in `src/.gitignore` fails with `unknown-item` carrying `unsupported-gitignore` at line 1.
  - Kept from the first round: the cold and warm scope tests and `kept_contents_are_chosen_by_stat_not_by_time`.
  - The cold control's `project validate` gets the test's own `TMPDIR`.
- No failing test exists for two of the fixes, because no counter or output observes what they remove. The stat through a symbolic link: no syscall tracer is available here (strace and bpftrace are absent, and `perf trace` lacks permission). The cost of `retain`: only a timing test would show it, and the project rejects those.
- Docs: DES-CODE-READERS, DES-DAEMON-SESSIONS (including the watch bullet), VER-CODE-WORK-COUNTERS (the listing-sentinel sentence, a row for a directory at the path, the zero-work warm row), VER-CODE-MARKERS (the hidden-path row). The comment in `warm.rs` is updated.
- Checks on 2933d24 plus this change: fmt, clippy (warnings denied), `cargo test --workspace` (26 suites), `project validate` (0 errors, 1 warning from before this change), release performance test (32 passed), wasm32 build of the engine, `zban check`. All pass.

## Decisions made

- Which kept record answers is decided by stat: the record a check would keep (its stat is the file's and its check settled it), with no order of records. This covers code contents (from `get` or a scan) and directory listings and `.gitignore` files (from the inventory or from `get`). The choice is made where the walk reaches each one, after its parent admitted it. Written in DES-DAEMON-SESSIONS.
- `get` keeps its listings in a session map apart from the inventory, not watched. Every walk reuses and refreshes them, and a listing of the code files drops the directories it does not reach. Written in DES-DAEMON-SESSIONS.
- `get` forgets kept code only for its own path. Another file that left stays until the next code listing or a `get` of that path: it costs memory, never a stale answer. Written in DES-DAEMON-SESSIONS.
- No counter for directory listings or watches. VER-CODE-WORK-COUNTERS says a listing shows only where it reads a `.gitignore` or reports a problem.
- Decided by the orchestrator: a not-found `get` of a path that a code include pattern matches carries the diagnostics of the check on the way, after its own. Such a path is never a document or an ID, so that `get` always fails, and no success case needs a rule. Written in DES-CODE-READERS.
