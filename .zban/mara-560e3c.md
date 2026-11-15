---
title: item update --clear-field refuses a field whose rows start the table
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-w0xasr
created_at: 2026-11-15T15:42:18Z
---

Goal: `item update --clear-field <name>` removes every row of an optional field also when its rows start a hand-written metadata table.

Context: found in the re-check of mara-w0xasr. Mara writes `Type` as the header row, so this needs a hand-written table whose header row is a repeated optional field. The result is a refusal, not lost data. `crates/mara-engine/src/authoring/update.rs`: removing the header row with `write::remove_row` promotes the next row into its place, and the next row is another row of the cleared field, whose own removal overlaps that splice; the overlap check then refuses the update.

Reproduction:

```markdown
| Owner | Ann |
| --- | --- |
| Owner | Bob |
| Type | Note |
```

`mara item update NOTE-ALPHA --clear-field owner` exits 1 with `invalid-item`, "the changes of this update overlap in the item's table", and changes nothing.

- [ ] Remove a run of rows that starts at the header with one splice that promotes the first row that stays, and add the case to the clearing test in `crates/mara/tests/authoring/update.rs`
