---
title: Place mention text in the rename write order
status: done
type: task
related:
  - mara-esxqzv
discovered_from: mara-4y2cp4
created_at: 2026-11-15T09:08:12Z
closed_at: 2026-11-15T10:03:34Z
---

Goal: an interrupted `item rename` never leaves a mention whose text is still the old ID after the old ID has stopped resolving.

Context: REQ-RENAME-ID (docs/authoring.mara.md) rewrites every mention whose text is exactly the old ID. DES-WRITE-PROCEDURE (docs/writes.mara.md) orders a rename as code markers, then the `ID` row, then inbound link text, and a rerun completes it only while the old ID resolves. If mention text is rewritten after the `ID` row, a rename stopped there leaves mentions with the old ID: mention text is free (DES-RELATION-LINKS), so no diagnostic reports them, fix does not rewrite them, and a rerun no longer works. VER-INTERRUPTED-EDITS (docs/verification-writes.mara.md) checks the states that REQ-INTERRUPTED-EDIT-LOSES-NOTHING lists, so it does not catch this.

- [x] Decide where mention text goes in the rename order. Rewriting it with the code markers, before the `ID` row, keeps it inside the window in which a rerun completes the rename
- [x] Record the order in DES-WRITE-PROCEDURE

## Summary of Changes

Decided by the maintainer, option "Mentions before the ID row (Recommended)": "Order becomes: code markers and mention text, then the `ID` row, then inbound relation link text. A rename stopped before the `ID` row is completed by a rerun; one stopped after it leaves only stale relation link text, which is reported and which fix rewrites. No stale mention can remain unnoticed. Changes the Accepted order by one step."

Recorded in DES-WRITE-PROCEDURE (rename row of the order table) and ADR-MENTIONS-BEFORE-ID-ROW, both in docs/writes.mara.md. VER-INTERRUPTED-EDITS and VER-RENAME-ID check it, and mara-esxqzv's test item points at them. Done in the follow-up commit to mara-4y2cp4 on the branch docs/alpha2-verification-items, as the orchestrator authorized. Checks: `project validate` and `zban check` exit 0.
