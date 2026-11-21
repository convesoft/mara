---
title: The code tests build on macOS, and their daemon helper waits once for its daemon to stop
status: todo
type: bug
parent: mara-xzfeyj
related:
  - mara-j6vpbt
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:11Z
---

Goal: `cargo test --workspace` compiles on macOS again, and the code tests do not wait twice for a daemon that has stopped.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); the orchestrator read the code and confirms both. Locations are as of that commit. mara-9fphtg (commit `254a511`) made every code test also run through a daemon of the test. In `crates/mara/tests/code/main.rs` the test binary is `#![cfg(unix)]`; `struct Daemon` (near 391) and `mod daemons` (near 13) are `#[cfg(target_os = "linux")]`, and `through_daemon` has a stub for other systems (near 252), but the field `Project.daemon: RefCell<Option<Daemon>>` (near 91), its initialiser (near 138) and `impl Drop for Project` (near 94) are not gated, so the binary does not compile on macOS, a supported platform (ADR-KEEP-PLATFORMS-AND-NODE). `Daemon::stop` (near 483) calls `self.daemons.kill()` and then drops `self`, whose `Daemons::drop` (`crates/mara/tests/support/daemons.rs`, near 88) calls `kill()` again; each `kill()` waits until 500 ms pass without a daemon, so every code test that used a daemon waits twice. A test-only change; no macOS machine is at hand, so the gating is checked by reading and by `cargo check --tests --target` for an Apple target if the toolchain has one.

- [ ] The daemon field, its initialiser and the `Drop` of `Project` are compiled on Linux only, or `Daemon` has a stub on other systems; no item of `crates/mara/tests/code/` names a Linux-only type outside a Linux-only item
- [ ] A stopped daemon is waited for once
