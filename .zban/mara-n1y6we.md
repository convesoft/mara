---
title: Bound the daemon's connections and report readiness only once it serves
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-17gykj
created_at: 2026-11-16T18:59:04Z
---

Goal: a misbehaving or numerous client cannot keep the daemon alive, delay `stop` or starve other clients, and `mara daemon start` reports success only for a daemon that serves.

Context: found in the review of mara-17gykj (DES-DAEMON-LIFECYCLE in docs/daemon.mara.md; `crates/mara/src/cli/daemon/server.rs`). The reviewer's probes are `/home/araketski/.tmp/codex/rv-load-probe.py` and `rv-cleanup-probe.py` (read-only).

- A client that pipelines about 1,200 requests without reading replies keeps the daemon alive past its idle period, and delays `stop` by the 10 s write timeout (`connection`, the write of each answer).
- Connections have no admission limit: with `RLIMIT_NOFILE=64`, 75 silent connections make the next request fail after 10 s (`accept`).
- Readiness is reported before the service threads exist: under a 20 MiB address-space limit, `start` reports success for a daemon that has already exited (`child`, then `serve`).

- [ ] Decide the limits (pending answers, connections) and how a client learns of a refusal
- [ ] Report `ready` only after the signal and accept threads run
- [ ] Tests for each case above
