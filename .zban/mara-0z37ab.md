---
title: "VER-FIX-CASES: project convert leaves fixable problems as they were"
status: todo
type: task
parent: mara-6k50w0
blocked_by:
  - mara-wjy3sh
discovered_from: mara-pvqpse
created_at: 2026-11-15T15:20:51Z
---

Goal: the last row of VER-FIX-CASES (docs/verification-writes.mara.md) passes for `project convert`. The row verifies the last sentence of REQ-ONE-FIX-OPERATION (docs/authoring.mara.md): no operation but fix repairs source.

Context: the other write operations already have this row.
- `item create`, `item update`, `item delete`, `relation add` and `relation remove`: `each_operation_changes_only_the_spans_it_requires` in `crates/mara/tests/authoring/writes.rs` keeps `docs/broken.mara.md` byte-identical.
- `item move` and `item rename`: `leaves_the_problems_it_does_not_edit` in `crates/mara/tests/relink/safety.rs` keeps the same diagnostics and bytes.

The conversion's input is a 0.3.0 project, so the problems that fix repairs exist only where converted documents sit beside documents already in the new format.

- [ ] Run `project convert` with the real CLI on a 0.3.0 project that also holds, in a document it does not convert, each problem fix repairs (DES-FIX-REPAIRS in docs/authoring-requests.mara.md):
  - a heading and table without an anchor
  - a bare-ID relation value
  - a stale link path in a relation and in a mention
  - stale relation link text
  - a `missing-separator`
  - an anchor without its blank line
  - a setext heading in an item's heading place

  Afterwards those problems stay as they were, byte for byte. If a 0.3.0 project cannot hold such a document, record that the row does not apply to the conversion instead.
