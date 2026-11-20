---
title: "Client routing: on-demand start, first call in-process"
status: done
type: feature
parent: mara-0mxw21
blocked_by:
  - mara-17gykj
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T20:47:15Z
---

Context: REQ-FIRST-CALL-IN-PROCESS and DES-DAEMON-LIFECYCLE in docs/daemon.mara.md. The MCP server becomes a client of the daemon, so the "Each is a cold request" bullet of DES-MCP-SERVER (docs/hosts.mara.md) changes with this issue.

- [x] CLI and MCP clients start the daemon in the background and serve the current request in-process
- [x] No start with `CI`, `MARA_NO_DAEMON=1` or the direct-mode flag
- [x] A dispatched request is never replayed
- [x] Test: the other cases of VER-DAEMON-START, and the SIGKILL cases of VER-DAEMON-WRITES (docs/verification-daemon.mara.md)

Note from mara-gfb6n6 (2026-11-16): `query execute`, `query view` and `query gate` take `--input <name>=@<file>`. Once a client sends an operation to the daemon, such a path is resolved from the client's working directory, not the daemon's (read it in the client, or send the working directory with the request, as for `--project`).

## Summary of Changes

- `crates/mara/src/cli/client.rs` (new): the CLI and `mara mcp` are clients of the daemon. A document operation, except `project init` and `project convert`, goes to the daemon of the build as an `operation` request. If no daemon runs, the client spawns `mara daemon start` detached and serves the request in-process. In direct mode (`CI`, `MARA_NO_DAEMON=1`, `--no-daemon`) it does neither. The daemon side, `client::answer`, checks the arguments again and runs the same function in a `Context` with the client's working directory. It answers with the printed text, whether it failed, the exit status and the work counts, which the client reports as its own.
- `cli.rs`: `Context` carries the client's working directory and direct mode. `Reply` is what a request prints. `--no-daemon` is a global flag. `--project`, `--input <name>=@<file>` (`query.rs`), `prime`, `project init` and `project convert` resolve from `Context::cwd`, not from the process's directory.
- `cli/mcp.rs`: each tool call goes through `client::request` from the server's working directory. `mara mcp --no-daemon` serves every call in-process.
- `cli/daemon/server.rs`: an `operation` arm in `Daemon::answer`. Operation requests count as activity for the idle period at their start and end. A panic in an answer still ends the request's count, and the client gets `daemon-lost`. The daemon moves to `/` when it detaches.
- `rules.rs`: Soufflé's child gets an empty signal mask, since the daemon blocks SIGTERM, SIGINT and SIGHUP in every thread and now runs evaluations.
- Tests: `tests/daemon.rs` became `tests/daemon/main.rs` with `client.rs` (8 cases of VER-DAEMON-START and a `daemon-lost` read), `mcp.rs` (the MCP cases and MCP direct mode) and `writes.rs` (the two SIGKILL cases of VER-DAEMON-WRITES and one write completed through the daemon). A call served by the daemon is shown by stopping the daemon with SIGSTOP: the call waits. A call served in-process ends while the only daemon is stopped. Every other test that runs `mara` sets `MARA_NO_DAEMON=1`. In the performance harness, `run_with` sets it unless the case gives `XDG_RUNTIME_DIR`. Mutations checked: never dispatching, replaying a lost request, resolving from the daemon's directory and ignoring `CI` each fail tests.
- Docs: DES-DAEMON-LIFECYCLE (dispatch, refusal, request contents, start, direct mode), DES-OPERATIONS (`--no-daemon`), DES-MCP-SERVER (each call is a client request, not a cold request).
- Checks, each exit 0 at the head (the integration tip has not moved since the branch was cut): cargo fmt --check; clippy with warnings denied; cargo test --workspace --locked (with no daemon left after the run and nothing created in `/run/user/1000/mara`); `MARA_NO_DAEMON=1 cargo run … project validate` (valid, 0 errors, 0 warnings); the release performance test (19 passed, 2 ignored); the wasm32 build; zban check.

## Decisions made

- The direct-mode flag is the global flag `--no-daemon`. `CI` counts when set to a non-empty value, and `MARA_NO_DAEMON` only when it is exactly `1`.
- A request is dispatched once its frame is written in full. A refusal from the daemon (`stopping`, another build, a malformed request) means it ran nothing, so the client serves the request in-process. If the runtime directory or socket fails its checks, the client serves in-process and starts no daemon. A failure after dispatch, including an unreadable answer, is `daemon-lost` with exit status 1 and no work counted.
- The client spawns `daemon start` in its own working directory and in a process group of its own, with `--project` made absolute when given. It does not wait for it, and a thread reaps it so that the MCP server leaves no zombie. A failed start does not fail the request.
- The request carries the operation's words, arguments, options, the MCP inputs, `project`, `cwd` and `json`. The daemon resolves every path from `cwd` and runs in `/`.
- Until sessions exist, the daemon serves each request from the files as they are, and reports the counts of that request.

## Review fixes

- Client's umask. New module `crates/mara/src/caller.rs` holds `Caller` (the client's umask and temporary directory). The CLI reads it before any thread starts, and the MCP server keeps that copy. A request carries it, and the daemon runs the operation inside `caller::serve`, which sets it for the serving thread only; the daemon's umask never changes. Operations create files only through `caller::create_dir`, `create_new`, `write` and `tempdir`. Each creates with the mode less the client's umask, then sets exactly that mode; with no client set (direct mode), the process umask applies as before. This covers the write procedure's directories and temporary files (`item create`, `move`, `rename`, `project fix` and every other write), and the evaluation's private directory, its files and output directory. Soufflé's child sets the client's umask in `pre_exec`. Existing files keep their mode, as in direct mode. Test: `caller::a_write_through_the_daemon_gives_files_the_modes_of_the_clients_umask` (daemon umask 022, client umasks 077 and 027, create into a new directory and move into a new directory, against direct mode; a document of mode 0604 keeps it).
- Client's temporary directory. The request carries `TMPDIR`, or `/tmp`, made absolute from the client's directory, and evaluations create their private directory there. Operations read no other value of the environment; `MARA_TEST_PAUSE` stays the daemon's, as VER-DAEMON-WRITES has it. DES-DAEMON-LIFECYCLE lists the values a request carries: working directory, umask, temporary directory. Test: `caller::an_evaluation_through_the_daemon_uses_the_clients_temporary_directory_and_umask` (daemon started with a `TMPDIR` then removed: a query through the daemon equals direct mode; a slow query's private directory lies in the client's `TMPDIR` with modes 0700/0600; the daemon's Soufflé child has `Umask: 0077`).
- The first call never waits. In `client::a_read_while_the_lock_is_held_runs_in_process`, the call and both its pipes end within 3 s while the test holds the lock, and a second open of the lock still cannot take it. The first-read case has the same bound. A client that waited for its `daemon start` fails it after 10 s.
- AGENTS.md: the `project validate` local check and the worker's `cargo run --quiet --bin mara` line use `--no-daemon`.
- Each new test was checked to fail with its fix removed (no umask override, no temporary-directory override, no Soufflé umask, a client that waits for `daemon start`).

## Decisions made in the review round

- Orchestrator, on the maintainer's behalf: the client's wait on a daemon that stopped answering (no read or connect deadline) moved to mara-xqcnqw as a checklist item, with the note that an incomplete send fell back only after 20.5 s.
- Orchestrator, on the maintainer's behalf: the AGENTS.md local check and worker calls run in direct mode (`--no-daemon`), so that no worktree's build leaves a daemon in the developer's session.
- Worker: the client's values are kept per serving thread (`caller::serve`) rather than through `unshare(CLONE_FS)` per thread, which is Linux-only. A created file or directory gets its mode less the client's umask at creation, so it never has more bits than direct mode would give it, and then exactly that mode.
