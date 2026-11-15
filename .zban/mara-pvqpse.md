---
title: Fix operation
status: in-progress
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-ONE-FIX-OPERATION in docs/authoring.mara.md; incomplete items in DES-ITEM-STRUCTURE; link diagnostics in DES-RELATION-LINKS.

- [ ] Adds missing anchors and rewrites bare-ID relation values, stale link paths and text, malformed item formatting; never deletes an item or supplies a heading, type or ID
- [ ] Rewrites the stale links an interrupted move or rename leaves
- [ ] Changes nothing in a valid project
- [ ] VER-FIX-CASES passes, with the other cases of docs/verification-writes.mara.md for `project fix`; work-counter and scaling tests, the hub case included (VER-WORK-COUNTERS, VER-SCALING, DES-PERFORMANCE-HARNESS)
