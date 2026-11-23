---
title: A file system whose clock runs behind the checker's can make an unsettled record look settled
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-efyg6p
created_at: 2026-11-23T08:30:38Z
---

Goal: a session never reuses old contents or an old listing because the file system's timestamps run on a clock behind the machine that checks them, or decide and document that Mara does not support such a file system.

Context: found in the review of mara-efyg6p. The 2 s rule of DES-INPUTS-AND-KEYS (docs/engine.mara.md) compares a file's mtime and ctime, set by the file system, with the request's `now`, taken from the checking machine's wall clock (`tracked::Stat::settled_at` in `crates/mara/src/tracked.rs`; `now` from `SystemTime::now()` in `crates/mara/src/cli/client.rs` and `crates/mara/src/session.rs`). The item now states the assumption: timestamps advance on the checker's wall clock and a timestamp tick is shorter than 2 s. A shared file system whose server clock is seconds behind the daemon's host breaks it, for example NFS. DES-DAEMON-SESSIONS (docs/daemon/index.mara.md) discusses changes from another machine on a network file system, so such a project is plausible.

The reviewer's sequence, not exercised on a real mount:

1. The server's clock is more than 2 s behind the daemon's host, and its timestamps have a 1 ms tick.
2. A file is written on the share.
3. A warm request reads it. Its check time is more than 2 s past the file's mtime and ctime, so the record counts as settled.
4. The file is overwritten in place with contents of the same size, within the same server tick.
5. Any later request sees the same size, mtime, ctime and inode, and reuses the old contents, however long after. A directory listing whose stat collides the same way keeps an old entry list.

Options to weigh:
- Compare with the file system's clock, for example the timestamps of a file Mara writes, or a newer timestamp seen in the same directory, instead of the wall clock.
- Treat a record as settled only after a second check that sees the same stat at least 2 s later by the file system's clock.
- Document that such file systems are unsupported.

- [ ] Reproduce on a mount whose server clock is behind, or show why it cannot happen
- [ ] Decide with the maintainer which option, and update DES-INPUTS-AND-KEYS
