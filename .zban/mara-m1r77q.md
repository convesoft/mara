---
title: project fix can leave a duplicate-row when two relation rows of an item name the same target
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-pvqpse
created_at: 2026-11-15T15:20:55Z
---

Found while building `project fix` (mara-pvqpse). It is rare: an author has to write the same relation twice in different forms.

Case: one item has `| Refines | REQ-B |` and `| Refines | [REQ-OLD](old/b.mara.md#<MID of REQ-B>) |`. Fix rewrites both rows to the canonical link `[REQ-B](b.mara.md#<MID>)` (DES-FIX-REPAIRS in docs/authoring-requests.mara.md). Validation then reports `duplicate-row` at the second row, a problem the project did not have before.

- [ ] Decide whether fix leaves such a row unrepaired, or removes the repeat (fix never deletes an item, but a row is not an item), and record it in DES-FIX-REPAIRS
