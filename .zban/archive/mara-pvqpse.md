---
title: Fix operation
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T15:21:27Z
---

Context: REQ-ONE-FIX-OPERATION in docs/authoring.mara.md; incomplete items in DES-ITEM-STRUCTURE; link diagnostics in DES-RELATION-LINKS.

- [x] Adds missing anchors and rewrites bare-ID relation values, stale link paths and text, malformed item formatting; never deletes an item or supplies a heading, type or ID
- [x] Rewrites the stale links an interrupted move or rename leaves
- [x] Changes nothing in a valid project
- [x] VER-FIX-CASES passes, with the other cases of docs/verification-writes.mara.md for `project fix`; work-counter and scaling tests, the hub case included (VER-WORK-COUNTERS, VER-SCALING, DES-PERFORMANCE-HARNESS)

## Summary of Changes

- `project fix` (`crates/mara/src/cli/fix.rs`): select the root, take the write lock, load the project, refuse `invalid-project` when the configuration or schema has an error, take the snapshot, plan, read back each changed document, then `write::apply`. A valid project is not written at all. The result is bounded like a read's inventory diagnostics.
- Repairs are planned in the engine (`crates/mara-engine/src/fix.rs`, no I/O, the host supplies MIDs): missing anchors, `missing-separator`, an anchor's missing blank line, one-line setext headings in an item's heading place, bare-ID relation values, stale relation links (written canonically), and stale mention paths, inline or through a link reference definition. Edits that overlap or touch become one splice.
- Parser: `Document::anchorless` marks regions that lack only their anchor; `PARSER_VERSION` is 11. `validation::{is_id, resolve}` are now `pub(crate)` for fix.
- Docs: new DES-FIX-REPAIRS in docs/authoring.mara.md; a Fix row in the write order table of DES-WRITE-PROCEDURE.
- Tests:
  - `crates/mara/tests/fix.rs`, 14 cases through the real CLI. It covers every VER-FIX-CASES row but the last (filed as mara-0z37ab) and, for `project fix`, VER-PRESERVED-BYTES. It also covers VER-REFUSED-EDITS: configuration and schema errors, undeclared arguments, the step-2 pause for the only file and for the first and a later file, and the first file's step-3 pause. Then VER-INTERRUPTED-EDITS, every file × the four ways, with `RLIMIT_FSIZE`, and the lock case of VER-CONCURRENT-WRITERS.
  - Engine tests for the destination locators and `anchorless`.
  - Performance: the `project fix` corpus case (it writes nothing) and `moved_hub_fix_counts`. The latter checks the exact cold write row, then a second fix with nothing to write. The hub fix scaling case uses `check_growth_prepared`, which restores the written files outside the timed span, and `Hub::move_hub`.
- End to end: on a copy of this repository's docs, fix found nothing to do. After `format.mara.md` was moved to another directory by hand, it rewrote 13 stale links in 8 files and the project validated clean.

Checks, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (11 passed, 2 ignored, load 2.6), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

- Fix repairs exactly the problems VER-FIX-CASES lists, as the table of DES-FIX-REPAIRS gives them. Other malformed formatting stays reported: an anchor merged after text, a setext heading over several lines, HTML in the anchor's place, malformed rows. This follows the docs, then the simplest option.
- A relation link with a stale path or text, and a bare-ID value, is rewritten as the writer writes every relation link (DES-ITEM-WRITER): `[ID](path#MID)`, with the target's ID, or the link's text when the target has none.
- A mention keeps its text and form: only the path bytes change. For a reference link, fix rewrites the path in its definition, in one splice with every link and image that uses it, as DES-ITEM-WRITER requires. A destination written with backslash escapes or character references is left as it is and stays reported, which DES-FIX-REPAIRS states.
- A missing anchor goes in an `Expect::Bytes` splice over the whole region, as the writer's own test writes it, with a new MID that no item of the project has. `Expect::Item` would refuse a region whose body uses a link reference definition outside it.
- Documents are written in path order, and a rerun completes an interrupted fix (DES-WRITE-PROCEDURE, Fix row).
- A document whose repairs would not read back refuses the whole fix with `unreadable-edit`, as for every write operation.
- Result: `format_version`, `repairs` (`code`, `path`, `line` before the fix, `message`), the first that fit in 8,192 bytes of JSON as for a read's inventory diagnostics, and `repairs_total`. The human form prints `<path>:<line>: fixed[<code>]: <message>`, then the counts. Exit status 0.
- An item that gets its anchor is repaired as fully as one that has it, in the same run, and a bare ID may name it (changed in review, finding 3; see `## Review`).
- Fix finds what to repair from the snapshot and runs no built-in checks, so it counts no check steps. Its write row is the cold row of VER-WORK-COUNTERS: a cold request plus 1 parse per distinct content written.

## Rejected approaches

- Detecting regions that lack only their anchor, and missing separators, from the text alone. A `---` inside a block quote ends its region, but a text scan does not see it as a separator and would write a `---` into a valid file. The parser marks such regions instead, and missing separators come from its diagnostics.

## Review

The review of `0f9cd2b` reported three realistic findings and one exotic. The orchestrator triaged them under the maintainer's rules of 2026-11-15. One fix commit on top of `0f9cd2b`.

- Finding 3, fixed: combined repairs needed several runs. A hand-written item without an anchor whose relations are bare IDs got only its anchor from the first fix.
  - The parser now keeps, for each region that lacks only its anchor, the heading and the metadata rows it already reads (`Document::anchorless: Vec<Anchorless>`), so no parse is added and the counter row stays as decided.
  - The planner draws every new anchor's MID before planning any document. It repairs the relation rows of an item that gets its anchor like those of any item: bare IDs, stale relation links. It also converts that item's one-line setext heading. A bare ID may name an item that gets its anchor in the same run, which covers two such items naming each other and an existing item naming one.
  - Relation rows of every item now go through one path (`Planner::relations`), and edges only carry mentions.
  - Test `repairs_items_that_get_their_anchor_in_one_run` (`crates/mara/tests/fix.rs`) runs the real CLI on two items without anchors that name each other by ID, one with a setext heading, a bare ID and a stale relation link to existing items, a stale mention, and an existing item whose bare ID names one of them. One fix makes 10 repairs, `project validate` reports nothing, and a second fix finds nothing. With the two new paths disabled the test fails.
  - Checked as part of the same defect: an item missing both its separator and its anchor is no reported problem, since its heading and table read as narrative or as another item's body (DES-ITEM-STRUCTURE), so fix has nothing to repair there. A stale mention in an item that gets its anchor was already repaired in the same run, and the new test covers it.
  - DES-FIX-REPAIRS: the "waits for the next run" sentence is replaced by "An item that gets its anchor has its heading, relation values and mentions repaired in the same run, and a bare ID may name it".
- Finding 1, docs only: DES-ITEM-STRUCTURE now says that fix adds the missing anchor only when nothing stands in the anchor's place, and that a paragraph of inline HTML there stays reported. The code is unchanged.
- Finding 2 and the exotic finding (stale mentions whose destination is written with escapes, or whose title repeats the destination syntax): filed together as backlog issue mara-rhaycg and left alone.

Checks at the new head, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked`, `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

### Decisions made in review

- Decided by the orchestrator under the maintainer's rules of 2026-11-15 for exotic findings, for the maintainer's slice review: fix adds a missing anchor only when nothing stands in the anchor's place. A region whose anchor paragraph is other inline HTML, such as an anchor with a lowercase MID, stays reported (DES-ITEM-STRUCTURE). Adding a second anchor with a new MID next to a damaged one would break every link that still uses the old MID.
- An item that gets its anchor is repaired as fully as one that has it, in the same run, without an extra parse: the parser keeps the rows and heading it already reads for such a region, and the new anchors' MIDs are drawn before planning.

## Rebase onto fb875de

After `item move`/`item rename` (mara-esxqzv) and the single-file operations (mara-w0xasr) landed, overlaps were resolved by keeping the tip's mechanism and deleting this branch's duplicate:

- Performance harness: `check_growth_prepared` is gone. The hub fix scaling case uses the tip's `check_writes` and `restore.rs`, and `moved_hub_fix_counts` uses `restore::settle` and `Written::learn`. `Hub::move_hub` now only moves the hub's document. The `project fix` corpus case stays `Reads::Documents`, since the corpus is valid and fix writes nothing.
- `cli.rs`: the tip's `author` and `relink` modules and this branch's `fix`, with all operations in the list. `validation.rs` and `checks.rs` are the tip's (`pub use is_id`, `pub(crate) use resolve`).
- Docs: DES-FIX-REPAIRS moved to `docs/authoring-requests.mara.md`, since `docs/authoring.mara.md` would have exceeded 200 lines beside DES-MOVE-AND-RENAME. Its links, the Fix row of the write order table in DES-WRITE-PROCEDURE, the index entry of that page and the module comments point there. DES-OPERATIONS needed no change.
- Parser: the tip still had `PARSER_VERSION` 10. This branch's 11 for `Document::anchorless` stays.
- New test `fixes_what_a_rename_stopped_after_its_id_row_leaves`: a real `item rename` stopped at the pause of the first file after the `ID` row leaves stale relation link text in two files, and one fix makes the project valid.
- mara-0z37ab is narrowed to `project convert` and blocked by mara-wjy3sh only. The landed operations' tests cover the row (`crates/mara/tests/authoring/writes.rs`, `crates/mara/tests/relink/safety.rs`), with small differences in problem kinds. The authoring fixture has no stale mention path, and its setext heading is in a body. The relink fixture has no stale relation link text and no anchor that lacks only its blank line.
