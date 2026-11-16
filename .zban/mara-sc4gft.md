---
title: A hand-written item without its separator stays silent when its table has only a Type row or starts with another row
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-msymag
created_at: 2026-11-16T16:00:00Z
---

Goal: decide whether the shapes that mara-msymag leaves unreported need a diagnostic.

Context: after mara-msymag (and its review), a heading that is not in its region's heading place, directly followed by a table whose first row is a `Type` row and that has an `ID` or a `Status` row, whatever their values, is `missing-separator`, also after a paragraph of inline HTML (DES-REGIONS in docs/format.mara.md). Still silent, after another item and without `---`, so an update of the item before replaces the text:

- A heading and a table with only a `Type` row, neither `ID` nor `Status`. Left out so that a body table of types with a `Type` column, such as `| Type | Requirement |` and no rows, is not read as an item.
- A heading and an item's table whose first row is not the `Type` row, such as a `Key | Value` header followed by `Type`, `ID` and `Status` rows.
- A heading after a paragraph that holds a merged MID anchor: reported as `merged-anchor` only.

- [ ] Decide whether any of these is realistic enough to report, and how without false alarms on body tables
