---
title: "Daemon housekeeping: watcher lock, one parse per frame, lock file name, WorkCounts sum"
status: todo
type: task
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
---

Goal: five small defects of the daemon's plumbing are gone.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. They are grouped because each is small and they touch neighbouring code; split one off if it grows. Items: DES-DAEMON-SESSIONS and DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md), REQ-READS-DURING-VALIDATION and DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- [ ] `crates/mara/src/watch.rs`, `linux::Inner::sync`: the watcher's state lock is held while `symlink_metadata` and `inotify_add_watch` run for every directory, which blocks the thread that reads events and the `sync` and `forget` of other sessions. Make the system calls outside the lock, or keep the locked part short
- [ ] `crates/mara/src/cli/daemon/server.rs:455`, `connection`: each frame is parsed with `serde_json::from_slice(&frame)` to look at `request` and at `client::writes`, thrown away, and parsed again in `daemon.answer(&frame)`. Parse once and pass the value
- [ ] `crates/mara/src/daemon.rs`: `create_lock` names its temporary file by PID only, so two threads of one process can collide; `write_frame` copies the whole payload into a second `Vec` where it could write the 4-byte length and then the payload
- [ ] `crates/mara/src/session.rs` and `crates/mara/src/cli/client.rs` each add the eight fields of `WorkCounts` by hand: one `AddAssign` (or an `add` method) on `WorkCounts` in `mara-engine` replaces both
- [ ] `crates/mara/src/caller.rs`: the audit noted remaining umask and mode edge cases without naming them here; whoever starts this issue gets the list from the audit, or drops this item if there is none

Done when the items are done, tests cover the ones with behaviour (the watcher lock, the lock file name), and the local checks pass with unchanged work counts.
