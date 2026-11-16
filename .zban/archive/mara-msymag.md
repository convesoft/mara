---
title: A hand-written item without a separator before it is read as the previous item's body, with no diagnostic
status: done
type: bug
discovered_from: mara-18193r
created_at: 2026-11-16T10:50:38Z
closed_at: 2026-11-16T16:00:19Z
---

Goal: an item written by hand directly after another item, without the `---` line before it, is reported, so that it does not silently drop out of the project.

Context: found while writing the agent guide (mara-18193r). In a disposable project with an item at the end of `docs/a.mara.md`, append by hand, without `---`:

```
## Passkey verification

| Type | Verification |
| --- | --- |
| ID | VER-PASSKEY |
| Status | Draft |
| Verifies | REQ-PHISHING-RESISTANT-AUTH |

Text.
```

`mara project validate` reports `valid: 0 errors, 0 warnings`, `mara project fix` finds nothing, and `item move VER-PASSKEY ...` fails with `unknown-item`. With `---` before the heading, validation reports `incomplete-item` and one `project fix` repairs it. The rules of "Documents and regions" in docs/format.mara.md ("An item ends only at a thematic break or end of file. Headings never end an item") make the text body of the item before, so no rule covers it. Agents append items this way; the guide (mara-18193r) tells them to put `---` first, but a forgotten separator loses the item without a word.

Changing it adds a diagnostic (and perhaps a `project fix` repair) to Accepted items in docs/format.mara.md and docs/authoring-requests.mara.md (DES-FIX-REPAIRS): a maintainer decision.

- [x] Decide (decided by the orchestrator on 2026-11-16, below): a heading followed by a table with a `Type` row inside an item body is reported (for example `missing-separator` at the heading), and whether `project fix` inserts the `---`
- [x] Implement it with its validation and fix tests

Decided by the orchestrator on the maintainer's behalf on 2026-11-16, resting on REQ-REPORT-EVERY-PROBLEM and REQ-ONE-FIX-OPERATION: the problem gets a diagnostic, and `project fix` repairs it. The worker of this issue designs the rule so that an ordinary table with a `Type` column in an item's body is not mistaken for an item. Until then the agent guide (`mara prime`, mara-18193r) tells agents to write the `---` before the item first.

Decided by the orchestrator on 2026-11-16, after the review of mara-18193r showed that the same silent case also happens to a first item written after a title and introduction: this issue covers every heading followed by a `Type` table that is not the first block of its region, not only an item after another item, and it starts as soon as mara-18193r is integrated.

## Summary of Changes

- Parser (`crates/mara-engine/src/document/region.rs`): a top-level heading that is not in its region's heading place, directly followed by a table with a `Type` row and an `ID` row whose value has the form of an ID, is `missing-separator` at the heading. Reading starts a new region there, as for a misplaced anchor, so the item before ends at the heading and the new region is the existing anchorless case (`incomplete-item`, `Document::anchorless`). Not reported when the block before the heading holds a MID anchor or is a paragraph of inline HTML. Tables after a heading are now read into rows wherever they may be such an item (still in the one event pass). `PARSER_VERSION` 12.
- Fix (`crates/mara-engine/src/fix.rs`): for an anchorless region flagged `missing-separator` at its heading, the `---` (with a blank line before it when the line before is not blank) goes in the anchor's splice, as one insertion, so the region's relation repairs still merge with it. One run leaves the project valid.
- Docs: DES-REGIONS (the rule, what is not reported, the way out for a false alarm, items now also end at a missing separator), DES-ITEM-STRUCTURE (the heading's place, fix adds `---` and anchor), DES-FIX-REPAIRS (the `missing-separator` row), DES-AUTHORING-REQUESTS (a body with such a heading is refused with `unreadable-edit`, checked by hand), VER-FIX-CASES (a new row). The agent guide keeps the "put a `---`" sentence and now says a forgotten one is reported and `project fix` adds it.
- Tests: `crates/mara/tests/fix.rs` `repairs_items_written_without_their_separator` through the real CLI, both shapes of the issue, with a bare ID naming the new item, a mention in the item before, and six body tables that must stay unreported (a `Type`/`Meaning` table with an `ID` row that is no ID, a table after a paragraph, in a list, a quote and code); exact bytes after one fix, `project validate` clean, a second fix finds nothing. Engine test `reports_an_item_heading_that_is_not_the_first_block_of_its_region`. Two existing expectations changed because the case is now reported: an opening `<a id>` tag in a paragraph before a heading and item table (`recovers_only_from_an_exact_anchor_at_its_place`), and `Text first.` before a heading and item table in `marks_the_regions_that_lack_only_their_anchor`.
- End to end: this repository's `docs/` and the 0.3.0 docs (`d7f0e73`) converted with `project convert` (27 files) both validate with no diagnostic. A real `item update --body` holding a heading and an item table is refused with `unreadable-edit`.

Checks, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (16 passed, 2 ignored), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

- The rule needs an `ID` row whose value has the form of DES-ITEM-IDS as well as the `Type` row. A body table of field types with `Type` and `Meaning` columns has a `Type` row, so the `Type` row alone would raise false alarms, and fix would turn such a table into an item. A declared type cannot count: the parser does not know the schema (DES-INPUTS-AND-KEYS), and the split must happen in the parser.
- The parser splits the region at the heading, as it does at a misplaced anchor. So the item before already ends there, and `item update`/`item delete` of it cannot overwrite the hand-written item's text. The new region reports `incomplete-item` too, since it also lacks its anchor; one fix repairs both.
- Code `missing-separator`, an error, at the heading's line. The message tells the user to put a `---` line before the heading, and, for a table that is no item, to put a paragraph between the heading and the table; DES-REGIONS names the same way out.
- A heading after a block that holds a MID anchor or is a paragraph of inline HTML is not reported: that block stands in the anchor's place, fix adds no anchor there (DES-ITEM-STRUCTURE), and a merged anchor stays reported only as `merged-anchor`. Filed with the missing-`ID`-row case as backlog issue mara-sc4gft.
- Fix inserts the `---` only together with the anchor that the same region gets, in one splice.

## Review

The review of `03e0bed` reported four realistic findings; the orchestrator decided them (below). One fix commit on top of the rebased branch (onto `a352c17`; no conflict, `PARSER_VERSION` stays 12, one above the tip's 11).

- Finding 1, fixed: an item with a typo in its ID, or without an `ID` row, stayed silent, and an update of the item before deleted it. The table rule is now: its first row is a `Type` row, and it has an `ID` or a `Status` row, whatever their values (`is_item_table` in `crates/mara-engine/src/document/region.rs`). Validation then reports the item's own problem (`invalid-id`, `missing-row`); fix adds the `---` and the anchor and supplies or corrects no ID.
- Finding 2, fixed: a paragraph of inline HTML right before the heading, such as a custom anchor, suppressed detection. Reading now starts the new region at that paragraph and reports `missing-separator` there. The parser records each such place (`Document::unseparated`), and `project fix` inserts only the `---` before the HTML; the region then reads as an incomplete item whose anchor fix does not add (mara-pvqpse), so `incomplete-item` stays, and a second fix finds nothing. Only a block that holds a MID anchor keeps a heading from being reported.
- Finding 3, docs: DES-REGIONS says that a heading followed by an item's table is an item wherever it stands, and that an example of an item goes in a fenced code block or gets a paragraph between heading and table. The `missing-separator` message names both ways out: a `---` before the heading (or before the HTML above it) if it is an item; a fenced code block, or a paragraph between heading and table, if it is an example.
- Finding 4, fixed: a refused `item update --body` named `empty-body` instead of the cause. The read-back (`crates/mara-engine/src/write/read_back.rs`, `problems`) now names a new `missing-separator` before any other new problem; `item create` already did. DES-AUTHORING-REQUESTS says so.
- A table of field types whose first row is `Type | Meaning` and that has a row whose first cell is `ID` or `Status` is reported, as the rule says: by shape it is an item of type `Meaning` with a malformed ID, and the message names the way out. A table with only a `Type` row is not reported.
- Tests through the real CLI, each failing without the change (checked: 0 of 4 pass with the source files of `a2cc75c`): `separates_an_item_with_a_malformed_or_missing_id`, `separates_an_item_after_custom_html_and_adds_no_anchor` (both: validation before, `item update REQ-OLD --body` keeps the section, one fix with exact bytes, validation after, a second fix finds nothing), `a_field_table_with_an_id_or_status_row_has_an_item_shape`, `a_refused_body_names_the_item_it_holds` (two updates and a create). The six unreported tables of `repairs_items_written_without_their_separator` stay; its `Fields` table no longer has an `ID` row, and a table with only a `Type` row is added. Engine test cases cover the same shapes.
- Docs: DES-REGIONS, DES-ITEM-STRUCTURE, DES-FIX-REPAIRS, DES-AUTHORING-REQUESTS, VER-FIX-CASES (two rows). The agent guide is unchanged.
- This repository's `docs/` and the 0.3.0 docs converted with `project convert` (27 files) validate with no diagnostic.
- mara-sc4gft is narrowed to what stays silent: a table with only a `Type` row, and an item table whose first row is not `Type`.

Checks at the new head, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked`, `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

### Decisions made in review

- Decided by the orchestrator on the maintainer's behalf: an item's table, for a heading that is not its region's first block, is one whose first row is a `Type` row and that has an `ID` or a `Status` row, whatever their values. This replaces the ID-form `ID` row decided above.
- Decided by the orchestrator on the maintainer's behalf: a paragraph of inline HTML right before such a heading goes with it. `missing-separator` is reported there and fix adds only the `---`, never an anchor next to custom HTML (mara-pvqpse). This replaces the exclusion of such headings decided above.
- Decided by the orchestrator on the maintainer's behalf: a heading followed by an item's table is an item wherever it stands, plain rendered examples included; an example goes in a fenced code block or gets a paragraph between heading and table. Undo: a new issue that adds an explicit marker for examples.
- A table of field types with a `Type | Meaning` header and a row whose first cell is `ID` or `Status` is reported: it has an item's shape, and nothing without the schema tells it apart from an item with a malformed ID.
- A refused edit names a new `missing-separator` before the other new problems, since it causes them.
