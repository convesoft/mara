---
title: "Daemon housekeeping: watcher lock, one parse per frame, lock file name, WorkCounts sum, caller umask and mode"
status: in-progress
type: task
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
---

Goal: seven small defects of the daemon's plumbing and of the caller's identity are gone.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. They are grouped because each is small and they touch neighbouring code; split one off if it grows. Items: DES-DAEMON-SESSIONS and DES-DAEMON-LIFECYCLE (docs/daemon/index.mara.md), REQ-READS-DURING-VALIDATION and DES-PERFORMANCE-HARNESS (docs/performance.mara.md).

- [ ] `crates/mara/src/watch.rs`, `linux::Inner::sync`: the watcher's state lock is held while `symlink_metadata` and `inotify_add_watch` run for every directory, which blocks the thread that reads events and the `sync` and `forget` of other sessions. Make the system calls outside the lock, or keep the locked part short
- [ ] `crates/mara/src/cli/daemon/server.rs:455`, `connection`: each frame is parsed with `serde_json::from_slice(&frame)` to look at `request` and at `client::writes`, thrown away, and parsed again in `daemon.answer(&frame)`. Parse once and pass the value
- [ ] `crates/mara/src/daemon.rs`: `create_lock` names its temporary file by PID only, so two threads of one process can collide; `write_frame` copies the whole payload into a second `Vec` where it could write the 4-byte length and then the payload
- [ ] `crates/mara/src/session.rs` and `crates/mara/src/cli/client.rs` each add the eight fields of `WorkCounts` by hand: one `AddAssign` (or an `add` method) on `WorkCounts` in `mara-engine` replaces both
- [ ] `crates/mara/src/caller.rs:31-45`, `Caller::of_this_process`: it reads the umask by setting it (`libc::umask(0o077)`) and setting it back, which changes process-wide state twice and races with any other thread that creates a file meanwhile, for example when Mara runs inside a multi-threaded host such as the Node library of alpha.6. On Linux, read `Umask:` from `/proc/self/status`; say what other platforms do
- [ ] `crates/mara/src/caller.rs:54-64`, `serve`: when it ends it sets the thread's `SERVED` to `None` where it should restore the caller that was there before, so a nested `serve` loses the outer caller
- [ ] `crates/mara/src/caller.rs:114-127`, `write`: it asks `fs::symlink_metadata(path).is_ok()` and then opens with `create(true)`, and decides from the first answer whether to set the file's permissions; a file created or removed between the two gets the wrong decision. Decide from the open itself (`create_new`, then fall back to opening the existing file)

Done when the items are done, tests cover the ones with behaviour (the watcher lock, the lock file name), and the local checks pass with unchanged work counts.
