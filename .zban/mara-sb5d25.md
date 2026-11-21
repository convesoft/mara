---
title: "One rule for 'is this file unchanged': the grammar registry and the session's Soufflé identity follow DES-INPUTS-AND-KEYS"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: every place that decides whether a file is unchanged uses the rule of DES-INPUTS-AND-KEYS (docs/engine.mara.md) through one shared type.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: seam. `tracked::Stat` (`crates/mara/src/tracked.rs`) implements the rule: size, mtime, ctime, device and inode, `symlink_metadata`, and the 2 s settle rule. Three others differ:
- `crates/mara/src/grammar.rs` has its own `Stat { length, modified, inode }` without ctime, device or the settle rule, and stats the grammar's files before it reads them, so a file changed during the load, or within the same timestamp tick with the same length, stays loaded.
- `session::Build` (`crates/mara/src/session.rs`) identifies the Soufflé executable by path, size and mtime (backlog mara-je202a holds the cases this misses).
- `crates/mara/src/write.rs` computes device and inode again as `identity()`.
- `tracked::list` takes no `Keep`, so the background derivation and the write hand-over list unsettled directories again where they skip unsettled files.
The daemon's build key (`crates/mara/src/daemon.rs`) differs on purpose (ADR-DAEMON-PER-VERSION).

- [ ] `tracked::Stat` and its settle test are the one seam; the grammar registry, `session::Build` and `write::identity` use it
- [ ] `tracked::list` takes `Keep`
- [ ] mara-je202a closed or narrowed to what remains
