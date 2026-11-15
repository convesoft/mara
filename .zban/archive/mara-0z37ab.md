---
title: "VER-FIX-CASES: project convert leaves fixable problems as they were"
status: done
type: task
parent: mara-6k50w0
blocked_by:
  - mara-wjy3sh
discovered_from: mara-pvqpse
created_at: 2026-11-15T15:20:51Z
closed_at: 2026-11-15T16:58:52Z
---

Goal: the last row of VER-FIX-CASES (docs/verification-writes.mara.md) passes for `project convert`. The row verifies the last sentence of REQ-ONE-FIX-OPERATION (docs/authoring.mara.md): no operation but fix repairs source.

Context: the other write operations already have this row.
- `item create`, `item update`, `item delete`, `relation add` and `relation remove`: `each_operation_changes_only_the_spans_it_requires` in `crates/mara/tests/authoring/writes.rs` keeps `docs/broken.mara.md` byte-identical.
- `item move` and `item rename`: `leaves_the_problems_it_does_not_edit` in `crates/mara/tests/relink/safety.rs` keeps the same diagnostics and bytes.

The conversion's input is a 0.3.0 project, so the problems that fix repairs exist only where converted documents sit beside documents already in the new format.

- [x] Run `project convert` with the real CLI on a 0.3.0 project that also holds, in a document it does not convert, each problem fix repairs (DES-FIX-REPAIRS in docs/authoring-requests.mara.md):
  - a heading and table without an anchor
  - a bare-ID relation value
  - a stale link path in a relation and in a mention
  - stale relation link text
  - a `missing-separator`
  - an anchor without its blank line
  - a setext heading in an item's heading place

  Afterwards those problems stay as they were, byte for byte. If a 0.3.0 project cannot hold such a document, record that the row does not apply to the conversion instead.

## Summary of Changes

The row applies: a 0.3.0 project can hold a document already in the item format, written by hand or left by a stopped conversion. The conversion leaves such a document as it is. It holds items and no `:::mara` block, so the conversion changes it only by rows that other documents' relations give its items (DES-CONVERSION-MAPPING).

- Test `writes::leaves_the_problems_that_fix_repairs_as_they_were` in `crates/mara/tests/convert/writes.rs`: `project convert` with the real CLI on the mapping fixture of VER-CONVERSION plus `docs/broken.mara.md`, which holds each problem of DES-FIX-REPAIRS:
  - a heading and table without an anchor;
  - a bare-ID relation value;
  - a stale link path in a relation and in a mention;
  - stale relation link text;
  - a `missing-separator`;
  - an anchor without its blank line;
  - a setext heading in an item's heading place.
  Afterwards that document is byte-identical, and `project validate --path docs/broken.mara.md` still reports exactly `bare-id-value`, `incomplete-item`, `merged-anchor`, `missing-separator`, `setext-heading`, `stale-link-path` and `stale-link-text`.
- Done in the branch of mara-wjy3sh, after its rebase onto `052b7f5`. The checks are those of that branch.
