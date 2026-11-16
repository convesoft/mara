---
title: Keep a browser workspace's unexported edits when the engine traps
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-jptfbq
discovered_from: mara-274e1m
created_at: 2026-11-16T17:49:05Z
---

Goal: a panic of the WASM engine in the browser workspace's worker loses no unexported edit.

Context: DES-BROWSER-WORKSPACE (docs/hosts.mara.md) keeps the workspace in the worker that runs the engine. A Rust panic in `wasm32-unknown-unknown` traps, and the instance's memory cannot be trusted afterwards, so edits held only there are lost. DES-RULE-EXECUTION already keeps the workspace out of the Soufflé worker for the same reason. AGENTS.md: a failure preserves user data.

- [ ] Decide where the workspace's files live so that a trap loses none (for example in JavaScript beside the instance, with a fresh instance after a trap), and write it into DES-BROWSER-WORKSPACE
- [ ] Add the case to VER-BROWSER-WORKSPACE (docs/verification-browser.mara.md), with a way to make the engine trap that only a debug build has, as ADR-DEBUG-PAUSE-POINT allows
