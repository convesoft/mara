---
title: Run the souffle child process in an operating-system sandbox on Linux
status: done
type: feature
parent: mara-qzp1gd
blocked_by:
  - mara-bx82k0
discovered_from: mara-t2qrcb
created_at: 2026-11-16T10:42:28Z
closed_at: 2026-11-16T21:54:18Z
---

Goal: a program from an untrusted checkout cannot read or write files outside its private directory or run other code, even when it gets past the allow-list scan or hits a memory fault in Soufflé.

Context: RISK-SOUFFLE-REACHES-OUTSIDE (Accepted; Unlikely, Severe: level HIGH) in docs/rules-facts.mara.md threatens REQ-PROGRAMS-ONLY-COMPUTE (docs/rules.mara.md). The allow-list scan, the private directory and `-l ""` (DES-PROGRAM-PROFILE, DES-RULE-EXECUTION) mitigate it, but five reviews each found a way past earlier, weaker checks (EVD-SOUFFLE-PROGRAM-REACH in docs/rules-evaluation.mara.md), and a memory fault in Soufflé is outside any scan. mara-bx82k0 builds the child-process evaluation that this issue wraps.

Provisional decision, by the orchestrator on 2026-11-16 under the maintainer's rule of that day (it decides on the maintainer's behalf; security matters go ahead on the recommendation, marked provisional, when they can be undone). The question was mara-t2qrcb: an operating-system sandbox, or an accepted Decision that accepts the risk. Chosen: the sandbox, on Linux. Reasons: an Accepted risk of level HIGH has to be treated or accepted, accepting it loosens a security requirement and is the maintainer's alone to choose, and programs come with checkouts the user did not write. The maintainer may overturn this; undo by cancelling this issue and recording a Decision that `Accepts` the risk. It is listed in mara-46j65e.

- [x] A Design item for the sandbox around the `souffle` child process on Linux (Landlock, no new privileges; decide what the child may read, write and execute, and what happens on a kernel without Landlock), with a Decision item for the choice and the rejected alternative (accepting the risk)
- [x] The implementation, and tests that a program which reaches outside, by a planted way past the scan, is stopped by the sandbox and reported as a fault (REQ-PROGRAM-FAULTS-REPORTED)
- [x] The sandbox part of the open question removed from docs/rules-facts.mara.md; RISK-SOUFFLE-REACHES-OUTSIDE names the sandbox as a mitigation
- [x] macOS (Seatbelt) is not part of this issue: it goes with mara-ag4ees

Confirmed by the maintainer on 2026-11-16, so the decision above is no longer provisional. Chosen option, word for word: "Sandbox on Linux (Recommended) — Keep my decision: a Landlock sandbox around the child process on Linux, built before 0.4.0; macOS follows with the macOS work. Costs one issue of release work, and closes the risk instead of accepting it." Not chosen: accepting the risk; a sandbox after 0.4.0. The question and answer are recorded in mara-46j65e.

## Summary of Changes

- `crates/mara/src/rules/sandbox.rs` (Linux only): before each Soufflé start the parent builds a Landlock ruleset that handles every right of the kernel's ABI, with rules for the private directory (read), its `out/` (create, write, truncate), the Soufflé binary and its ELF interpreter (execute, read), the interpreter's directory (read) and `/etc/ld.so.cache`; and a seccomp filter for sockets, `io_uring_setup`, signals to other processes, `truncate`, mode, owner, time and xattr changes by path, keyrings, and foreign architectures or x32. The child sets no-new-privileges, enters the domain and the filter in a second `pre_exec` after `limit()`: system calls only, no allocation. No Landlock (or no filter for the architecture): the evaluation is a host fault naming Landlock, and Soufflé never starts.
- `rules.rs`: `spawn_and_wait` builds and applies the sandbox; `evaluate` is split into the preparation and `run_in`, so tests can plant text in the wrapper.
- Tests (`crates/mara/src/rules/sandbox/tests.rs`), each on the kernel's ABI (10 here) and with the ABI capped at 1 by a cap that only the unit tests contain: planted `.input` of a secret (fault), planted `.output` outside as text and JSON (nothing written, the program's rows), outputs only in `out/`; a `cc`-built helper in Soufflé's place that reads, writes, executes, opens network and UNIX connections and signals other processes (each a runtime fault, nothing outside changed or received); the helper without attempts passes; ABI capped at 0 refuses with 0 spawns. Removing the seccomp call fails 4 of them, removing `landlock_restrict_self` fails 5.
- Docs: DES-SOUFFLE-SANDBOX (Satisfies REQ-PROGRAMS-ONLY-COMPUTE, Mitigates RISK-SOUFFLE-REACHES-OUTSIDE), ADR-SOUFFLE-SANDBOX, ADR-SANDBOX-REQUIRED, VER-SOUFFLE-SANDBOX; DES-RULE-EXECUTION and the risk name the sandbox; an EVD-SOUFFLE-PROGRAM-REACH row for Soufflé's silent skip of an output it cannot open; the open question removed; the Linux platform in DES-PACKAGES needs Landlock.
- Checks at the head rebased on `chore/clear-worktree-for-rewrite` (eaf83c8): fmt, clippy, `cargo test --workspace`, `project validate`, the release performance test, the `wasm32` build and `zban check` all exit 0.

## Decisions made

- Landlock plus a seccomp filter, through `libc` rather than the `landlock` crate; read access beneath the interpreter's directory rather than the exact libraries (ADR-SOUFFLE-SANDBOX).
- Without Landlock, Mara evaluates no program; ABI 1 suffices because the filter covers its gaps (ADR-SANDBOX-REQUIRED). Rejected: evaluating unsandboxed with or without a warning, requiring ABI 6/9/10, an opt-out option.
- Soufflé writes only beneath `out/`; the rest of the private directory is read-only to it.
- A planted output that Soufflé cannot open stays a passing result with the program's rows (Soufflé 2.5 skips it silently); the write is stopped. Follow-up: mara-k4g6y1 (backlog).
- Residual, documented in DES-SOUFFLE-SANDBOX: metadata reads outside (`stat`, `access`), and code mapped from memory running inside the same sandbox. Untested here: arm64 (no target installed); its system call constants were checked in `libc`.

## Fixes after review (2026-11-20)

1. Inherited descriptors: the child marks every descriptor from 3 up close-on-exec (`close_range`, `CLOSE_RANGE_CLOEXEC`) as the first sandbox step, so it holds only 0, 1 and 2 at `exec`; a refused call stops it before `exec`. Test `the_child_holds_no_inherited_descriptor`.
2. Metadata through a descriptor: the filter also refuses `fchmod`, `fchown`, `fsetxattr` and `fremovexattr` (`utimensat` and `fchmodat2` already covered their descriptor forms), and every `ioctl` but `TCGETS`, which covers `FS_IOC_SETFLAGS` and `FS_IOC_FSSETXATTR`. Test `the_child_cannot_write_outside` (descriptor forms on its own binary).
3. Signals through asynchronous I/O: `fcntl` with `F_SETOWN`, `F_SETOWN_EX` and `F_SETSIG` is refused, and the `ioctl` allow-list refuses `FIOSETOWN`, `SIOCSPGRP` and `FIOASYNC`. Test `the_child_cannot_signal_another_process`.
4. `socketpair` is refused; no other call yields a socket without an inherited one (`io_uring_setup` was already refused; inherited ones are closed by 1). Test `the_child_cannot_open_a_network_connection` (socket pair sending to a UNIX datagram and an abstract socket).
5. Libraries: the packages ship the Soufflé of `scripts/build-souffle.sh` (DES-PACKAGES, DES-SOUFFLE-BUILD), linked only against libc, libm and the loader, which glibc installs in one directory; Mara never runs another Soufflé. DES-SOUFFLE-SANDBOX states the one-directory limit, a loader failure is a fault that names the library, the loader's directory and the supported Soufflé, and wider layouts are backlog mara-kga2ga. Test `a_library_outside_the_loader_directory_is_named`.
6. Setup failures: the child reports the failed step in its error code, without allocating; the fault names the step, says Soufflé was not started and that a caller's confinement must permit `close_range`, no-new-privileges, Landlock and seccomp filters. Test `a_refused_setup_step_is_named` (an outer filter on the test's thread refusing each step).
- ADR-SANDBOX-REQUIRED records the maintainer's confirmation of 2026-11-20 and, read from published profiles, not run: Docker's default profile up to 20.10.7 and gVisor refuse Landlock, WSL 2 has it. ADR-SOUFFLE-SANDBOX records the one-directory, close-on-exec and `ioctl` allow-list choices.
- Removing each fix fails its test (checked by mutation runs).
