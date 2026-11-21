---
title: A file that is both a document and a program is read and hashed once per request
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: DES-INPUTS-AND-KEYS (docs/engine.mara.md) holds for a file with two roles: it is read and hashed once in a request.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: behavior. Exotic: it needs a file that matches both a document pattern and `[rules].programs`. In `crates/mara/src/inputs.rs`, `Walk::directory` calls `self.program(child)` and then `self.document(child)`; both go through `Walk::check`, which passes `share = false` to `tracked::check`, so the first read is not kept for the second.

- [ ] `Walk::check` shares the read when the file has both roles; a work-counter test
