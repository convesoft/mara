---
title: WriteLock::take reports a .mara that is no directory as a symbolic link
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-sj9jfq
created_at: 2026-11-15T15:12:10Z
---

Goal: the write lock names the real problem when `.mara` is a regular file.

Context: `WriteLock::take` in `crates/mara/src/write.rs` (DES-WRITE-PROCEDURE in docs/writes.mara.md) opens `.mara` and refuses with `symbolic-link` ("`.mara` is a symbolic link, and Mara follows none") when the opened entry `!is_dir()`, even when it is a regular file with the same device and inode as the `lstat` before. Selection only selects a root whose `.mara` is a directory, and `project init` refuses a `.mara` that is no directory with `existing-file` before it locks, so only a `.mara` replaced by a file between selection (or init's check) and the lock reaches this: the request is still refused, under the wrong code and message.

- [ ] Refuse a `.mara` that is no directory, but not a link, with `io-error` ("`.mara` is not a directory") at line 1 of `.mara/config.toml`, and keep `symbolic-link` for a link or an identity change
- [ ] A test below the CLI that locks a root whose `.mara` is a regular file
