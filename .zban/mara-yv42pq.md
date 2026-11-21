---
title: "Records a get keeps: what the review and the re-check of mara-h4z424 left"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-cytrb4
  - mara-2ndg14
  - mara-kg912v
discovered_from: mara-h4z424
created_at: 2026-11-21T19:13:34Z
---

Goal: the records that `get` of a code location keeps in a session cost no avoidable work and no unbounded memory in the cases below. None gives a stale answer: every answer checks the stat.

Context: findings of the review (at `2674624`) and the re-check (at `090826c`) of mara-h4z424 (Codex, 2026-11-21; logs `~/.tmp/codex/review-mara-h4z424-2674624/findings.md` and `~/.tmp/codex/review-mara-h4z424-090826c/findings.md`), rated work-only or exotic by the orchestrator and left out of that issue. Code: `crates/mara/src/inputs.rs` (`held`, `Walk::gitignore`, `keep_ancestors`, `refresh_ancestors`, `State::ancestors`), `crates/mara/src/tracked.rs`. Items: DES-INPUTS-AND-KEYS (docs/engine.mara.md, "Contents that hash as before keep what was derived from them"), DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), REQ-WORK-FOLLOWS-CHANGE (docs/performance.mara.md).

- [ ] A `.gitignore` edited and read by a `get` while unsettled: the next `get` in the 2 s after the edit compares its reread with the inventory's older record, not with the record the `get` kept, and compiles the same rules again (`held` falls back to the first record when neither holds; prefer the record whose stat is the file's). No counter shows it
- [ ] Kept records are bounded by the session's history, not by its current paths: a session that only runs `get` keeps the listings and compiled ignores of directories it no longer reaches until a listing of the code files or the idle period
- [ ] Two overlapping `get`s around a `.gitignore` edit: the older one can remove the record the newer one kept, which costs one more read later
- [ ] A `.gitignore` that a code include pattern also matches is read and hashed twice by a cold `get` of it (there before mara-h4z424)
