---
title: The dispatcher re-raises a core-dumping signal of mara on Node itself
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-svq540
created_at: 2026-11-22T21:39:15Z
---

Goal: when the native `mara` dies of a core-dumping signal (SIGSEGV, SIGABRT, SIGBUS), the user sees one crash, of `mara`, not a second one of `node`.

Context: `packages/mara-npm/bin/mara.cjs` ends itself by the signal that ended `mara`, as DES-PACKAGES requires ("or ends itself by the signal that ended it"). For a signal whose default action dumps core, `process.kill(process.pid, signal)` makes Node dump core too, so systemd-coredump records a crash of `node` beside that of `mara`. Found while writing the dispatcher; not seen in a run (no crash was provoked).

- [ ] Decide whether DES-PACKAGES keeps "ends itself by the signal" for core-dumping signals, or exits with 128 plus the signal's number for them
- [ ] Change the dispatcher and the item together
