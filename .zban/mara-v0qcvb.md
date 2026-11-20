---
title: A write through an unflushed shared memory mapping goes unseen until its timestamps change
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T19:11:18Z
---

Goal: decide whether Mara should find a change that keeps every stat field it compares, and how, or keep it as a stated limit.

Context: the review of mara-xqcnqw's rebase over mara-hk2arx. A file changed through a writable shared memory mapping (`mmap` with `MAP_SHARED`), not yet flushed (`msync`) or unmapped, can keep its size, mtime, ctime, device and inode until the kernel updates the timestamps. The unchanged check of DES-INPUTS-AND-KEYS (`crates/mara/src/tracked.rs`, `check`) then keeps the old contents: a daemon's session answered `verified: passed` while direct mode answered `failed`. The reproduction is `mmap-all.py` with its log `mmap-all.log` (and `mmap-stale.py`, `mmap-stale.log`) in the reviewer's directory `/home/araketski/.tmp/codex/merge-review.E3b0/`. Editors, Git and agents do not write this way; DES-INPUTS-AND-KEYS states the limit.

- [ ] Decide: keep the limit, or detect such writes (for instance inotify `IN_MODIFY` on a file whose stat is unchanged marks it for a re-read)
- [ ] If detected, a test with a shared mapping written without `msync`
