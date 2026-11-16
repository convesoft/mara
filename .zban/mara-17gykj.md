---
title: Daemon process, IPC and lifecycle
status: in-progress
type: feature
parent: mara-0mxw21
created_at: 2026-11-13T22:10:06Z
---

Context: DES-DAEMON-LIFECYCLE in docs/daemon.mara.md; ADR-ON-DEMAND-DAEMON.

- [ ] Runtime directory, single-instance lock, socket within the path limit
- [ ] One lock and socket per Mara version (ADR-DAEMON-PER-VERSION); idle exit
- [ ] `mara daemon status | start | stop`
- [ ] Test: the `mara daemon`, runtime-directory, lock and build cases of VER-DAEMON-START (docs/verification-daemon.mara.md) that need no on-demand start, through `mara daemon start`
