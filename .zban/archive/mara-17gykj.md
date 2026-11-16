---
title: Daemon process, IPC and lifecycle
status: done
type: feature
parent: mara-0mxw21
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T18:07:35Z
---

Context: DES-DAEMON-LIFECYCLE in docs/daemon.mara.md; ADR-ON-DEMAND-DAEMON.

- [x] Runtime directory, single-instance lock, socket within the path limit
- [x] One lock and socket per Mara version (ADR-DAEMON-PER-VERSION); idle exit
- [x] `mara daemon status | start | stop`
- [x] Test: the `mara daemon`, runtime-directory, lock and build cases of VER-DAEMON-START (docs/verification-daemon.mara.md) that need no on-demand start, through `mara daemon start`

## Summary of Changes

- `crates/mara/src/daemon.rs`: the build (version, executable path, size, mtime) and its key; the runtime directory, lock and socket with their ownership, mode and link checks; peer user IDs; the IPC frames.
- `crates/mara/src/cli/daemon.rs`: `mara daemon status | start | stop` and the client side of the requests (`status`, `stop`). `crates/mara/src/cli/daemon/server.rs`: the daemon process, forked by `daemon start` while it holds the lock; detaches, listens, answers each connection on its own thread, stops on `stop`, SIGTERM/SIGINT/SIGHUP or the idle period (30 min; `MARA_TEST_IDLE_SECONDS` in debug builds), removes its socket and lets running requests finish.
- `libc` moved from a dev-dependency to a dependency (user ID, peer credentials, `fork`, `setsid`, signals).
- Tests: `crates/mara/tests/daemon.rs` (12 cases of VER-DAEMON-START through `mara daemon start`: status/start/stop with and without a daemon, ten starts at once, a held lock, a lock held past the timeout, SIGKILL with a stale socket, SIGTERM, two copies and an in-place replacement, a base naming no directory or a file, a base too long, a 60-byte base, a base or `mara/` that others could reach or that is a link, idle exit); `tests/performance/daemon.rs` (counter row and scaling case of the `daemon` operations); `tests/performance/idle_period.rs` (release build ignores and does not contain `MARA_TEST_IDLE_SECONDS`). The MCP parity and guide tests list the `daemon` operations as no tools and outside the guide.
- Docs: DES-DAEMON-LIFECYCLE holds the decisions below; VER-WORK-COUNTERS gains the alpha.4 cold row of the `daemon` operations.
- Checks: cargo fmt, clippy with warnings denied, cargo test --workspace, project validate, the release performance test, the wasm32 build and zban check, all at the rebased head.

## Decisions made

- The runtime directory's base must be absolute, a directory and no symbolic link, owned by the user with mode 0700, or every call runs in direct mode; `mara/` must be the same, or the `daemon` operations fail with `io-error` and change nothing there (stricter than the design, which named only a missing base).
- Lock `<key>.lock` and socket `<key>.sock`, `<key>` the first 16 hex digits of BLAKE3 over the build. The lock: regular file, mode 0600, opened with `O_NOFOLLOW`, held with `flock` for the daemon's life, holding its PID. The socket: mode 0600; a stale one is removed only by the lock holder and only if it is a socket.
- The socket path with its NUL must fit `sun_path`: 108 bytes on Linux, 104 on macOS.
- IPC: 4-byte big-endian length plus a JSON object (max 64 MiB, `version` 1); each request carries the client's build key, which the daemon checks. Both ends drop a peer of another effective user ID.
- `daemon start` forks the daemon (setsid, stdio to /dev/null, other inherited descriptors closed) and returns once it listens. While another process holds the lock it waits up to 10 s for a daemon of its build, then fails with `io-error` naming the lock and its holder.
- Without a runtime directory, `start` fails with `io-error`; `status` and `stop` exit 0 reporting none. `CI` and `MARA_NO_DAEMON` do not affect the `daemon` operations.
- Without a session the daemon exits after the idle period of DES-IDLE-SESSIONS from its start or last request; `status` and `stop` do not count as requests. SIGINT and SIGHUP stop it as SIGTERM does.
- `status --format json`: `running`, `pid`, `sessions`, `socket`, `lock`, `unavailable`.

## Review fixes

- The lock has mode exactly 0600 whatever the umask: a missing lock is created under another name with that mode and hard-linked into place; an existing lock of another mode (or one that cannot be opened for writing) is refused with its mode named, never repaired. The socket is bound under umask 0177, set to 0600 and checked; a client refuses a socket of another mode or owner. Test: `modes_hold_whatever_the_umask_and_a_lock_of_another_mode_is_refused`.
- The base and `mara/` must have mode exactly 0700, special bits included (a real `/run/user/<uid>` is 0700 with none, as pam_systemd and the XDG Base Directory spec give it). Test: `a_runtime_directory_that_others_could_reach_is_refused` (bases of 1700 and 2700, `mara/` of 1700).
- A start that fails after binding its socket removes it. Test: `a_start_that_fails_after_binding_leaves_no_socket` (file size limit 0).
- Every test that starts a daemon kills the daemons of the test on drop, also after a panic, by the identification of VER-DAEMON-START, through one helper, `crates/mara/tests/support/daemons.rs`, used by `tests/daemon.rs` and the performance tests (now Linux only). Test: `a_failed_test_ends_its_daemons_and_only_those`.
- The comment on the daemon's signal mask is corrected; mara-xqcnqw holds the note on resetting Soufflé's mask and cancelling evaluations at shutdown.
- Filed to the backlog: mara-n1y6we (connection limits, pipelined requests, readiness before the service threads).
- DES-DAEMON-LIFECYCLE records the exact modes, the refused lock and the socket removal.
