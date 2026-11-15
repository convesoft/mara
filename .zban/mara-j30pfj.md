---
title: item move rewrites a link title that repeats the link's destination, not the destination
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-esxqzv
created_at: 2026-11-15T15:39:30Z
---

Goal: `item move` rewrites the destination of a link to the moved item also when the link's title holds the same text.

Context: found by the review of mara-esxqzv at `c69ab14` and rated exotic, so it went to the backlog under the maintainer's rules of 2026-11-15. Nothing here starts without the maintainer. Code: `crates/mara-engine/src/relink/links.rs:20`. Related: mara-rhaycg has the same shape in `project fix`.

Reviewer's reproduction: given `[REQ-X](a.mara.md#MID "(a.mara.md#MID)")`, moving REQ-X to `c.mara.md` rewrites the parenthesized URL in the title instead of the destination. The command succeeds; subsequent validation reports `stale-link-path`. This requires the unusual duplicated destination inside the title.

- [ ] The move rewrites the destination, and a test covers a title that repeats it
