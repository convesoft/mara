---
title: Daemon process, IPC and lifecycle
status: todo
type: feature
parent: mara-0mxw21
created_at: 2026-11-13T22:10:06Z
---

Context: DES-DAEMON-LIFECYCLE in docs/daemon.mara.md; ADR-ON-DEMAND-DAEMON.

- [ ] Runtime directory, single-instance lock, socket within the path limit
- [ ] One lock and socket per Mara version (ADR-DAEMON-PER-VERSION); idle exit
- [ ] `mara daemon status | start | stop`
