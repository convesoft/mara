---
title: The code tests build on macOS, and their daemon helper waits once for its daemon to stop
status: done
type: bug
parent: mara-xzfeyj
related:
  - mara-j6vpbt
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:11Z
closed_at: 2026-11-25T10:21:48Z
---

Goal: `cargo test --workspace` compiles on macOS again, and the code tests do not wait twice for a daemon that has stopped.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); the orchestrator read the code and confirms both. Locations are as of that commit. mara-9fphtg (commit `254a511`) made every code test also run through a daemon of the test. In `crates/mara/tests/code/main.rs` the test binary is `#![cfg(unix)]`; `struct Daemon` (near 391) and `mod daemons` (near 13) are `#[cfg(target_os = "linux")]`, and `through_daemon` has a stub for other systems (near 252), but the field `Project.daemon: RefCell<Option<Daemon>>` (near 91), its initialiser (near 138) and `impl Drop for Project` (near 94) are not gated, so the binary does not compile on macOS, a supported platform (ADR-KEEP-PLATFORMS-AND-NODE). `Daemon::stop` (near 483) calls `self.daemons.kill()` and then drops `self`, whose `Daemons::drop` (`crates/mara/tests/support/daemons.rs`, near 88) calls `kill()` again; each `kill()` waits until 500 ms pass without a daemon, so every code test that used a daemon waits twice. A test-only change; no macOS machine is at hand, so the gating is checked by reading and by `cargo check --tests --target` for an Apple target if the toolchain has one.

- [x] The daemon field, its initialiser and the `Drop` of `Project` are compiled on Linux only, or `Daemon` has a stub on other systems; no item of `crates/mara/tests/code/` names a Linux-only type outside a Linux-only item
- [x] A stopped daemon is waited for once

## Summary of Changes

- `crates/mara/tests/code/main.rs`: `Project.daemon`, its initialiser, `impl Drop for Project` and the `RefCell` import are `#[cfg(target_os = "linux")]`. Every item of `tests/code/` that names a Linux-only type (`Daemon`, `daemons::Daemons`, `restore`, `through_daemon`, the new test) is now Linux-only; `add.rs`, `edits.rs`, `facts.rs`, `grammar.rs`, `markers.rs` name none.
- `crates/mara/tests/support/daemons.rs`: a consuming `kill_once(self)` kills as `kill()` does and sets a flag that only it sets, so `Drop` does not wait again; `kill()` and `Drop` are otherwise unchanged, so the daemon and performance tests keep their deliberate second pass. `Daemon::stop` uses `kill_once`. A per-thread count of waits (`waits()`).
- New test `a_stopped_daemon_is_waited_for_once`: drops a project whose daemon served `project validate` and asserts one wait. With `Daemon::stop` back on `kill()` it fails with 2 against 1.
- Checks: `cargo check --tests --target aarch64-apple-darwin -p mara` stops in `ring`'s C build (no Apple SDK), so the gating was checked by reading. Full `pnpm run checks`: 863 passed, 0 failed, 16 skipped (831/14 workspace, 32/2 performance); `project validate` 0 errors, 0 warnings.

## Decisions made

- `kill_once` uses a flag rather than `std::mem::forget(self)`: `Daemons` owns two `PathBuf`s, which `forget` would leak, and a flag keeps `Drop` the one place that decides.
