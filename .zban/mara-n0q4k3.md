---
title: Exported facts clamp a line number above 2,147,483,647
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-y4asgm
created_at: 2026-11-16T15:20:03Z
---

Goal: a fact's line number is the document's line number, also beyond the signed 32-bit range, or the document is refused.

Context: found by the review of mara-y4asgm at `8511215` by inspection and rated exotic, so it went to the backlog under the maintainer's rules of 2026-11-15. Nothing here starts without the maintainer. `crates/mara-engine/src/facts.rs:115` silently clamps locations above `i32::MAX`. A document with over 2,147,483,647 lines can fit within the documented byte limit (4 GiB) but export an incorrect anchor or edge line. Not exercised at that size.

- [ ] Either refuse such a document or state the limit in DES-FACT-BRIDGE
