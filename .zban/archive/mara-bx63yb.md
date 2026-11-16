---
title: Write the Verification items for the alpha.4 requirements
status: done
type: task
priority: high
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-16T17:03:22Z
---

Goal: every requirement that the alpha.4 slice delivers has an accepted Verification item: the requirements that the issues of mara-0mxw21 name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [x] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [x] One Verification item for each, or one shared by requirements that a single check covers
- [x] The test items in those issues point at the Verification items instead of restating them

Started on 2026-11-16 before the alpha.3 epic closed, by the orchestrator's decision under the maintainer's rule of that day: this issue writes documents only, and writing it now shortens the path to alpha.4. The alpha.4 epic stays blocked by the alpha.3 epic.

## Summary of Changes

The slice delivers these requirements. Each now has an accepted Verification item:

| Requirement | Where the slice names it | Verification |
| --- | --- | --- |
| REQ-FIRST-CALL-IN-PROCESS | mara-jkabqy; DES-DAEMON-LIFECYCLE (mara-17gykj) | VER-DAEMON-START (new); the Node client's cases come with mara-vfkgbs (alpha.6) |
| REQ-NEVER-STALE | mara-xqcnqw; DES-DAEMON-SESSIONS | VER-NEVER-STALE (new) |
| REQ-READS-DURING-VALIDATION | mara-xqcnqw, mara-p27z9t; DES-DAEMON-SESSIONS | VER-READS-DURING-VALIDATION (new); its warm budget stays with VER-TIME-BUDGETS (Draft, mara-4dr2p2) |
| REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING | mara-p27z9t; the epic's done criterion | VER-WORK-COUNTERS: warm rows checked against today's operations and completed; VER-SCALING unchanged |
| REQ-SAME-RESULT-ON-EVERY-HOST | mara-wz40n6 | VER-HOST-COMPARISON: the daemon case now says what it sends and compares |
| REQ-WRITERS-SERIALIZED, REQ-REFUSED-EDIT-CHANGES-NOTHING, REQ-INTERRUPTED-EDIT-LOSES-NOTHING | DES-WRITE-PROCEDURE, which DES-DAEMON-SESSIONS names | VER-DAEMON-WRITES (new); VER-CONCURRENT-WRITERS' daemon row |
| REQ-CLI-MCP-PARITY | mara-jkabqy (the MCP server becomes a daemon client) | VER-HOST-COMPARISON's daemon case sends from `mara mcp` too; VER-CLI-MCP-PARITY unchanged |

Not in this slice: REQ-BROWSER-ACCESS-AUTHORIZED (mara-fkkcad, alpha.6), REQ-NODE-LIBRARY (mara-vfkgbs, alpha.6), REQ-TIME-BUDGETS (Draft; nightly run with mara-4dr2p2).

- New page docs/verification-daemon.mara.md, listed in docs/index.mara.md.
- DES-DAEMON-LIFECYCLE, ADR-DAEMON-PER-VERSION, DES-OPERATIONS (`daemon-lost`) and DES-PERFORMANCE-HARNESS hold the decisions below; ADR-TEST-IDLE-PERIOD and DES-IDLE-SESSIONS hold the answer on the idle period in tests.
- Test items in mara-17gykj, mara-jkabqy, mara-xqcnqw and mara-p27z9t now name the Verification items; mara-jkabqy notes that DES-MCP-SERVER's "Each is a cold request" changes with it.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (16 passed, on the second attempt after `berth` reported the machine busy), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked` and `zban check`: all exit 0. Items and relations were written with the worktree's binary (`item create`, `relation add`); `project fix` found nothing to fix; `relation list <REQ> --relation verified_by` shows the items above. Every page is within 200 lines.

## Decisions made

- Decided by the orchestrator on the maintainer's behalf (confirming my decision): a daemon belongs to a build, the version with the path, size and modification time of its executable, so that it never answers for another build's code; several worktrees of one version run different code every day (REQ-SAME-RESULT-ON-EVERY-HOST, REQ-NEVER-STALE; DES-DAEMON-LIFECYCLE, ADR-DAEMON-PER-VERSION). Rejected: the version alone. Undo: key on the version alone again.
- Runtime directory as in 0.3.0: `mara/` in `XDG_RUNTIME_DIR`, or `/run/user/<uid>` when unset, on Linux; in `TMPDIR` on macOS. A base that names no directory, or a socket path over the limit, gives direct mode.
- Clients send every document operation to the daemon except `project init` and `project convert`, which, like `prime`, run in the calling process.
- A client starts `mara daemon start` with its project as `--project`, detached from its standard streams; the daemon opens that session at once, so the next call is warm (ADR-ON-DEMAND-DAEMON). A daemon that opened no session exits after the idle period.
- `CI`, `MARA_NO_DAEMON=1` and the direct-mode flag mean direct mode: no connection and no start (the item said only "no start").
- A request whose daemon ends before answering fails with `daemon-lost` (DES-OPERATIONS) and is not replayed.
- `daemon status` prints whether the build's daemon runs, its process ID and its sessions' roots; `start` and `stop` exit 0 when there is nothing to do; `stop` returns once the daemon has exited. `stop` and SIGTERM let running writes finish and end the other requests.
- Debug stats of a daemon-served request: the session's work since its previous response, the session's opening and background re-derivation included; the client adds none; a daemon start is not counted (DES-PERFORMANCE-HARNESS). This makes a row's counts the same whether the watcher or the request did the work.
- The performance tests set `MARA_NO_DAEMON=1` in direct-mode cases and their own `XDG_RUNTIME_DIR` in daemon cases, so the empty environment never reaches the user's daemon.
- Warm rows run at 1×; each row starts settled (2 s after the last change). A repeated read does no counted work at all, which caches validation and query results by their keys (DES-INPUTS-AND-KEYS); the old "at most 1 souffle run" row is exact now. The code-scan row starts in alpha.5 (mara-779tms).
- Decided by the orchestrator on the maintainer's behalf (review), replacing my decision to keep VER-SCALING CLI-direct: the daemon gets scaling cases, since most requests take that path and near-linear growth is one of the reasons the project exists. Settled warm `search`, `item list` and `project validate`, and `search` and `item list` after one document edit, at the pull-request sizes and by the rule of VER-SCALING; the CLI-direct cases stay. Undo: drop the daemon scaling cases and rely on the CLI-direct ones. I chose the measurement (DES-PERFORMANCE-HARNESS, ADR-SCALING-CPU-TIME): the daemon's process CPU clock (`clock_getcpuclockid` on Linux, `proc_pid_rusage` on macOS), nanosecond resolution and including exited threads, read while the daemon is idle before the change and once it is idle again after the response, so watcher re-derivation counts in the run of the request that needed it. Rejected: `/proc/<pid>/stat` ticks (10 ms). The daemon cases run no program, since the clock excludes child processes; program runs stay timed in direct mode.
- Ten `item create` calls at once through one daemon must all succeed, since the daemon queues writes per project and computes each on the current files.
- Deterministic observation without new hooks: a running validation is a `souffle` process whose working directory is under the daemon's `TMPDIR` (as VER-EVALUATION-TIME-LIMIT already observes); daemons of a test are processes of the binary whose environment holds its runtime directory; concurrency with writes uses the approved pause points in the daemon's writer.
- That every request checks the files whether or not the watcher saw a change is also inspected at review of mara-xqcnqw: no test can blind the watcher deterministically.
- VER-CONCURRENT-WRITERS' daemon row uses two builds (copies of the binary), which a test can make, instead of another version.

- Decided by the orchestrator on the maintainer's behalf, as I recommended: a debug build reads the idle period of DES-IDLE-SESSIONS from `MARA_TEST_IDLE_SECONDS`, one variable for one period and no general test facility; a release build ignores it and does not contain it, which a case of the release-build `performance` test checks (DES-PERFORMANCE-HARNESS). It rests on ADR-DEBUG-PAUSE-POINT and on AGENTS.md's rule that a feature is done when the real workflow runs. Rejected: a user-visible `--idle` option of `daemon start`, and inspection only. Undo: remove the variable and verify the idle drop by inspection. Recorded in ADR-TEST-IDLE-PERIOD and DES-IDLE-SESSIONS; VER-NEVER-STALE has the idle-drop case; the Open questions bullet is gone; mara-xqcnqw's test item names both.

## Review

One review of `44652e8` found the coverage, the warm rows, the means of testing, the design changes and the form right, and reported four findings; the orchestrator decided each:

1. VER-NEVER-STALE did not check the state after an older evaluation finishes last: the V1/V2 case now replaces the long program with a fast one while V1 runs, answers V2 before V1 ends, and requires V3 after V1 to return the current direct-mode result without V1's result or fault.
2. CLI-direct scaling does not establish warm daemon scaling: VER-SCALING now has daemon cases (see Decisions made); DES-PERFORMANCE-HARNESS says how the daemon's CPU time is read and attributed; mara-p27z9t's test item names them.
3. Cold counters cannot show which process served a call: VER-DAEMON-START now defines "served in-process" by an observation a test can make (Test, not Inspection): cold counts, and no daemon of the test can have answered, because none runs or can start, or a settled running one would report nothing, or the daemon's first response afterwards reports the session's opening as cold counts too. Since a daemon reports a session's opening with its first response, that shows it had answered nothing before. The lock-held row and the `mara mcp` restart row use it.
4. Isolation on macOS: the test's runtime directory is `XDG_RUNTIME_DIR` on Linux and `TMPDIR` on macOS, set for every client and copy of the binary; tests identify and stop only daemons whose environment holds it (VER-DAEMON-START, VER-READS-DURING-VALIDATION, DES-PERFORMANCE-HARNESS).

Side note: VER-NEVER-STALE's reason for the Inspection now claims only that its cases do not keep the watcher from seeing their changes.

With mara-ttvp9z's notes: the code row of VER-WORK-COUNTERS reads "then a request that reads code", matching VER-CODE-WORK-COUNTERS, and DES-AGENT-GUIDE leaves the `daemon` operations, which are no tools, out of both guide renderings. The integration branch had not moved, so no rebase.

Checks after the fix, on a clean tree: `cargo fmt --all --check`, `cargo run --package mara --locked --quiet -- project validate` and `zban check` exit 0.
