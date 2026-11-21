---
title: "Watcher bookkeeping: orders that remove or leak a session's watches"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-ef6ybb
created_at: 2026-11-21T13:24:16Z
---

Goal: forgetting a dropped session never removes the watches of a newer session of the same project root.

Context: `wait_until_stopping` in `crates/mara/src/cli/daemon/server.rs` calls `Sessions::drop_idle`, then `Watcher::forget` for each dropped session. `linux::Inner::forget` in `crates/mara/src/watch.rs` removes the entry by root only. A request that opens a new session for the same root between those two calls, and syncs its watches, has them removed by the forget of the old session; the new session then goes unwatched until a later sync. Requests stay correct (every request checks the inputs on disk, REQ-NEVER-STALE in docs/daemon/index.mara.md); only background derivation is lost. Found while making `sync` run its system calls outside the lock (mara-ef6ybb), which compares session identity in `sync` but leaves `forget` as it was. The window is a few instructions wide.

- [ ] `forget` removes the entry only when its `Weak` points to the dropped session (`Weak::ptr_eq`)
- [ ] A test or an argument in the item shows a session reopened for the same root stays watched

## Orders found in the re-check of mara-ef6ybb (at `26dd9b2`)

Both concern the replacement of a watch in `linux::Inner::sync` (`crates/mara/src/watch.rs`, near line 240: the `gone` list and the per-directory remove, then add, steps). Each needs a second sync of the same root while the first runs, and both cost watch slots or background derivations only: requests still check the inputs on disk (REQ-NEVER-STALE). Review log: `~/.tmp/codex/review-mara-ef6ybb-26dd9b2/findings.md`.

- Leaked watch: sync A removes D's watch and releases the lock before its add. Sync B installs a replacement watch for D. An ancestor of D is then moved away and replaced, so D's old inode gets no self event. A takes the lock again, adds a watch for the new D and overwrites B's record for D without releasing B's descriptor or its root association. Nothing finds that descriptor again, `forget` included: it keeps a watch slot, and events from the moved tree can start unrelated background derivations.
- Missed removal: a sync computes its `gone` list once, at its start. A watch of a directory that the newer inventory no longer lists, recorded by a concurrent older sync after that list was taken, is not removed by the newer sync. It stays recorded, so it is not a dead descriptor, and the next sync removes it.

- [ ] A sync's add step that finds a record for its path published by another sync releases or keeps that watch, and never overwrites it unreleased
- [ ] A sync removes the watches of directories its inventory no longer lists also when they were recorded after it started (for example, the `gone` check made per step, or at the end under the lock)
