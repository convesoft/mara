---
title: "A write's hand-over to its session: an external save between rename and stat, and a file that is document and schema"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-8hw038
created_at: 2026-11-21T11:04:58Z
---

Goal: the snapshot a session derives from a write's hand-over equals the one a cold request would build, also in the two cases below.

Context: found by the review of mara-8hw038 (at `b7eece1`), both P3 and exotic; neither gives a stale response, because a written file counts as checked when the request began, so the next request's check reads and hashes it again. `Inputs::written` in `crates/mara/src/inputs.rs`; DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), DES-INPUTS-AND-KEYS (docs/engine.mara.md), DES-WRITE-PROCEDURE (docs/authoring/writes.mara.md).

- Another program saves the file between Mara's rename and the stat the hand-over takes: the session pairs Mara's parse with that save's stat until the next check reads the file.
- A file that is both a document and the schema: the hand-over puts its new document parse next to the old schema until the next check.

- [ ] Decide whether either needs closing (for example take the stat from the written file before the rename, and load a written schema again), and record it in DES-DAEMON-SESSIONS
