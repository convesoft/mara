---
title: project fix leaves a stale mention whose destination is written with escapes, or whose title repeats the destination
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-pvqpse
created_at: 2026-11-15T15:34:34Z
---

Found in the review of mara-pvqpse and rated exotic under the maintainer's rules of 2026-11-15. `project fix` finds a mention's destination in the source by comparing it with the destination that the parser resolved (`inline_destination` and `definition_destination` in `crates/mara-engine/src/fix.rs`). It leaves the mention stale, and still reported, when:

- the destination is written with backslash escapes or character references, such as `[t](a\_b.mara.md#<MID>)` or `[t](a&#46;mara.md#<MID>)`. DES-FIX-REPAIRS (docs/authoring-requests.mara.md) states this limit;
- the link's title repeats the destination syntax, such as `[t](a.mara.md#<MID> "](a.mara.md#<MID> )")`, so that two places could be the destination.

- [ ] Locate the written destination from the parser's events instead of by comparison, or keep the limit as stated
