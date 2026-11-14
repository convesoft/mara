---
title: Daemon loopback HTTP for browsers
status: todo
type: feature
parent: mara-xzfeyj
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-BROWSER-ACCESS-AUTHORIZED in docs/daemon.mara.md.

- [x] Decide how the listener is enabled and how a browser obtains the session token (DES-BROWSER-LISTENER)
- [ ] Loopback-only listener, started only by `mara daemon start --http`; port and token printed
- [ ] Session token, allowed origins and allowed projects checked before any project access
