---
title: A read after a killed daemon sometimes fails with daemon-lost instead of running in process
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-b76x4g
created_at: 2026-11-21T12:54:29Z
---

Goal: `client::after_a_daemon_is_killed_the_next_read_runs_in_process_and_starts_another` (`crates/mara/tests/daemon/client.rs:274`) passes every time.

Context: Seen once during `cargo test --workspace --locked` at `b7e4bc6` plus mara-b76x4g's change, which does not touch the daemon or `item list`. The next run passed. `item list` exited with status 1 and `daemon-lost`: "the daemon (process …) gave no answer to `item list`: its answer could not be read: Connection reset by peer (os error 104)". The read reached a daemon that was dying or had just been killed, and the test expects it to run in process. Either the client takes a reset on a fresh connection for a lost daemon, or the test kills the daemon while a connection is still being accepted.

- [ ] Find whether a reset before any byte of the answer can only come from a dying daemon, and if so, whether the client should run the read in process (the daemon-loss items in docs/daemon/)
- [ ] Make the test deterministic
