---
title: "Single-file item operations: create, update, delete, relations"
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T15:25:52Z
---

Context: docs/authoring.mara.md (REQ-CREATE-ITEM, REQ-UPDATE-ITEM, REQ-DELETE-ITEM, REQ-EDIT-RELATIONS).

- [x] Decide whether adding a relation accepts the inverse name: refused (REQ-EDIT-RELATIONS)
- [x] `item create` with ID proposal, `item update`, `item delete`
- [x] `relation add` and `relation remove`
- [x] VER-CREATE-ITEM, VER-UPDATE-ITEM, VER-DELETE-ITEM and VER-EDIT-RELATIONS pass (docs/verification-authoring.mara.md); for these operations, so do the cases of docs/verification-writes.mara.md, the CLI cases of VER-CONCURRENT-WRITERS, and the alpha.2 rows of VER-GITHUB-RENDERING, VER-MID-RESOLUTION (`item update`) and VER-NESTED-PROJECTS (docs/verification-documents.mara.md)
- [x] Work-counter and scaling tests for each operation: the alpha.2 cold rows of VER-WORK-COUNTERS and VER-SCALING (docs/verification.mara.md, DES-PERFORMANCE-HARNESS)

From mara-6ex559, which built the writer (`mara_engine::write`) and the write procedure (`mara::write`):

- [x] Complete the release-build case of DES-WRITE-PROCEDURE: in the release `performance` test, run `item create` with `MARA_TEST_PAUSE` naming a directory and require that it completes without creating `paused` there. The other half, `a_release_build_has_no_pause_points` in `crates/mara/tests/performance/pause_points.rs`, checks that the binary's bytes do not contain `MARA_TEST_PAUSE`. Until an operation calls `mara::write::apply`, the linker leaves the procedure out of every build, so that check holds trivially. It becomes meaningful with this issue.

## Summary of Changes

- Engine, `mara_engine::authoring` (no I/O): `create`, `update`, `delete`, `relation_add`, `relation_remove` over a snapshot. Each computes its splices with the item writer, reads the document back through one `ReadBack`, and returns the new contents (`Written`, `WrittenFile`) or an `EditError`. `Written::json` is the result for every host. `read::resolve` and `validation::is_id` are now `pub(crate)` re-exports for it.
- CLI, `crates/mara/src/cli/author.rs`: the five operations, run through the write procedure (lock, `read::open`, snapshot, compute, `write::apply`; nothing after the renames). `Inputs::admits` (from mara-esxqzv) checks that a new document would be an input. `read::open` is `pub(super)`.
- Tests, `crates/mara/tests/authoring/`: VER-CREATE-ITEM, VER-UPDATE-ITEM, VER-DELETE-ITEM, VER-EDIT-RELATIONS; VER-PRESERVED-BYTES (CRLF with BOM, front matter and no final newline at 0600, LF at 0644, irregular padding, emphasis, trailing spaces; plus a document with fix's problems that stays as it was, the other-operations row of VER-FIX-CASES); VER-REFUSED-EDITS (each operation's own refusals, broken schema, unknown and ambiguous item, undeclared option, and both pause points for every operation, a new document included); VER-CONCURRENT-WRITERS CLI cases (ten creates at once, a write behind a held lock); the alpha.2 rows of VER-GITHUB-RENDERING, VER-MID-RESOLUTION (`item update`) and VER-NESTED-PROJECTS.
- Performance: a write case per operation in `cli::cases` (`Reads::Writes`, from mara-esxqzv: the cold counts plus one parse per distinct content written, restored by `restore.rs`), each on its own document of the first two clusters. The corpus exposes `ids` and `untargeted`. The release `pause_points` case now runs `item create` with `MARA_TEST_PAUSE` set and requires no `paused`.
- Docs: DES-AUTHORING-REQUESTS in the new page `docs/authoring-requests.mara.md` (arguments, results, rules, five failure codes); DES-OPERATIONS points to its codes; VER-CREATE-ITEM's line case says the item starts after the separator the writer puts there.

Checks, all exit 0 after rebasing onto `c01393e`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (12 passed, 2 ignored, load 2.5), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

All in DES-AUTHORING-REQUESTS unless named.

- Arguments: `item create <type> <path> --title --body [--id] [--field <key>=<value>]... [--line <n>]`, `item update <item> [--title] [--field]... [--clear-field]... [--body]`, `item delete <item>`, `relation add|remove <source> <relation> <target>`, as in 0.3.0 where kept. An item is an ID, a MID or `<path>:<line>`, as reads name nodes. `Status` is a `--field` key (VER-UPDATE-ITEM counts it as a field value); `--field type=` and `--field id=` are rejected arguments. Rejected: 0.3.0's `--relation` on create and `-` for stdin, which REQ-CREATE-ITEM and REQ-UPDATE-ITEM do not ask for. 0.3.0's `--clear-field` is kept (review, below).
- Create and update refuse what validation would report of the values given (undeclared key, value outside the set, a relation as a field, several values of a single-valued key, a value twice) and, for create, a missing `Status`, required field or body. Simplest consistent reading of "creates an item that has no diagnostic"; rejected: writing anything and leaving it to validation.
- The ID proposal is prefix + the title's ASCII letters and digits in uppercase, other runs as one `-`; a proposal that is invalid or taken is refused, asking for `--id`. Rejected: appending a number, which hides the collision.
- A new item's heading level is that of the nearest item before, else after, else 2.
- `--line` takes 1 to the line after the last; the item goes at that line's start, after the separator that DES-ITEM-WRITER writes when the line before needs one. VER-CREATE-ITEM now says so.
- A new document is created only where it would be an input, checked by `Inputs::admits` as for a move; otherwise `not-an-input`, or `symbolic-link` for a link on its path.
- Update: a key's values replace its rows in order, surplus rows go from the last, extra values go after the last row that stays. An update that changes no byte writes nothing.
- After the read-back, create and update also refuse with `unreadable-edit` a body or title after which another item or narrative unit would start inside the written text, or the item would read a parse problem it lacked (`authoring::own_item`). The guard allows an inserted item that ends early (filed as mara-w8gafr).
- Delete counts relations and mentions from other items and from narrative, not the item's links to itself; each is a diagnostic of the refusal. The document stays, also when empty.
- `relation add` also refuses a row that is already there and a target without an ID; it does not refuse a cycle of an acyclic relation, which validation reports (REQ-EDIT-RELATIONS asks only for the type rule). `relation remove` deletes every row of that relation linking to the target's MID, whatever its path or text.
- Results: `format_version`, `item`, `files`; relations add `relation`, `target`, `line`.
- New failure codes: `invalid-item`, `inverse-relation`, `targeted-item`, `missing-relation`. A line inside an item or past the end is `invalid-position`, and a new document that would be no input `not-an-input`, the codes of DES-OPERATIONS that `item move` uses; `invalid-location` is gone (rebase, below).
- Limit stated, no code: a body that ends inside an open fenced code block is written when nothing follows the item; a later item after it is then refused until the fence is closed.

- Review, finding 2 (`--clear-field`, DES-AUTHORING-REQUESTS): `item update --clear-field <name>`, repeatable, removes every row of an optional field and changes no other byte; a field the item lacks changes nothing (`files` empty), as in 0.3.0. `Type`, `ID` and `Status` are rejected arguments (exit 2), since every item has them; so is a name that `--field` also sets in the same request (0.3.0: cannot also set that key), and an empty `--field` value, whose message now points to `--clear-field`. A required field, a relation (use `relation remove`) and a key the type does not declare are refused with `invalid-item`. Rejected: an empty `--field` value as the clearing route, because a value is never empty. VER-UPDATE-ITEM has a row for it.
- Review, finding 1 (DES-AUTHORING-REQUESTS): `relation remove` is refused with `invalid-item` when the source's type is no source of the relation (`Schema::relation_from`): its rows of that name are fields or undeclared keys, never the relation. `relation add` already refused this.

## Issues filed

- mara-w8gafr (backlog, deferred): the read-back guard accepts an inserted item that ends before its text; the operations already refuse it.
- mara-560e3c (backlog, deferred): `item update --clear-field` refuses a field whose rows start a hand-written table.

## Review

One review on `b8e9d59`; both findings fixed in one commit on top of it.

1. `relation remove` deleted a text field that shares the relation's name (`crates/mara-engine/src/authoring/relations.rs`): it now refuses with `invalid-item` and changes nothing when the source's type is no source of the relation. Test: `relations::leaves_a_field_alone_that_shares_a_relation_name` (remove and add on the reviewer's schema).
2. Clearing an optional field was dropped (`crates/mara/src/cli/author.rs`, `crates/mara-engine/src/authoring/update.rs`): `item update --clear-field <name>` is back. Tests: `update::clears_every_row_of_an_optional_field_and_nothing_else`, `update::refuses_to_clear_what_every_item_or_its_type_needs`.

Docs: DES-AUTHORING-REQUESTS (the `item update` row, the clearing rules, the relation-source rule) and a VER-UPDATE-ITEM row. Checks rerun at the new head with a clean tree, all exit 0.

## Rebase onto `9c8d2fc`

mara-esxqzv (`item move`, `item rename`) had solved two of the same needs. One mechanism is kept for each, the one on the tip:

- New document's path: `Inputs::admits` (`crates/mara/src/inputs/admit.rs`) replaces `Inputs::creatable`; `crates/mara/src/inputs/place.rs` is deleted. `item create` refuses with its codes, `not-an-input` and `symbolic-link`, and a line inside an item or past the end with `invalid-position` (`mara_engine::relink::INVALID_POSITION`). `invalid-location` is gone from the engine, the tests and DES-AUTHORING-REQUESTS, whose create rule now points to DES-MOVE-AND-RENAME.
- Performance harness: the tip's `Reads::Writes`, `restore.rs` and `check_growth_after` run the five write cases. `Case::writes`, `Case::restore`, `Case::settle`, `Reads::Write` and `check_growth_from` are gone. The corpus keeps `ids` and `untargeted`, which the cases name.
- `cli.rs` lists both operation sets; `inputs.rs`, `validation.rs` and `validation/checks.rs` are the tip's (its `pub use checks::is_id` serves the engine); DES-OPERATIONS and the docs index merged without conflict.

Also in this branch: mara-s55h22 (`relation add` given a renamed item's old ID fails with `unknown-item`, in the rename test), closed.

Re-check of `7c6a293`: both fixes verified. Its one remaining case, `--clear-field` of a field whose rows start a hand-written table, is a refusal, not lost data; filed as mara-560e3c (backlog, deferred), code left alone.
