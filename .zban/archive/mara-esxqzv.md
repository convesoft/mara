---
title: Move and rename across files
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T15:31:10Z
---

Context: REQ-MOVE-ITEM and REQ-RENAME-ID in docs/authoring.mara.md; REQ-INTERRUPTED-EDIT-LOSES-NOTHING, DES-WRITE-PROCEDURE and ADR-NO-MUTATION-JOURNAL.

- [x] Decide how the duplicate left by an interrupted move is resolved: rerunning the move (DES-WRITE-PROCEDURE)
- [x] `item move` and `item rename` in the documented write order
- [x] VER-MOVE-ITEM and VER-RENAME-ID (docs/verification-authoring.mara.md) and VER-INTERRUPTED-EDITS pass, with its four ways of stopping and its rename cases: code markers and mention text before the `ID` row, inbound relation link text after it (DES-WRITE-PROCEDURE, ADR-MENTIONS-BEFORE-ID-ROW, decided by the maintainer); for these operations, so do the other cases of docs/verification-writes.mara.md and the alpha.2 rows of VER-MID-RESOLUTION, VER-GITHUB-RENDERING and VER-NESTED-PROJECTS. The code-marker cases wait for mara-yhxq5x (alpha.5)
- [x] Work-counter and scaling tests for `item move` and `item rename`, the hub cases included (VER-WORK-COUNTERS, VER-SCALING, DES-PERFORMANCE-HARNESS)

## Summary of Changes

- Engine, `mara_engine::relink` (no I/O): `move_item` and `rename` compute each document's splices in the write order of DES-WRITE-PROCEDURE; `relink::links` finds the bytes of a link's path (inline or in its definition) and of its text, each checked against what the parser read. `validation::is_id` is public; `resolve` and `document::has_scheme` are visible in the crate.
- Host: `Inputs::admits` (`crates/mara/src/inputs/admit.rs`) says whether a new destination document would be an input, from the last listing. CLI `item move <id-or-mid> <path> [--line <n>]` and `item rename <id-or-mid> <new-id>` (`crates/mara/src/cli/relink.rs`): lock, read, compute, read back each document (`ReadBack`), `write::apply`.
- mara-jnvzxr done here: a move is refused with `unreadable-edit` at the reference when a link, image or footnote reference of the moved item uses a definition outside it.
- Docs: new DES-MOVE-AND-RENAME (authoring.mara.md); DES-WRITE-PROCEDURE (one write per file, at its first step; rerun wherever the copy is); DES-OPERATIONS codes `invalid-position`, `not-an-input`, `duplicate-id`/`wrong-id-prefix`/`missing-row`, `invalid-utf8` for edits.
- Tests: engine `relink::tests` and `relink::links::tests`; CLI `crates/mara/tests/relink/` (VER-MOVE-ITEM, VER-RENAME-ID, VER-PRESERVED-BYTES, VER-REFUSED-EDITS with both pause points, VER-INTERRUPTED-EDITS for every file and all four stops with reruns, the lock case of VER-CONCURRENT-WRITERS, the last row of VER-FIX-CASES, the alpha.2 rows of VER-MID-RESOLUTION, VER-GITHUB-RENDERING and VER-NESTED-PROJECTS); performance cases for move and rename on the corpus and the hub (`Reads::Writes`, `restore.rs`, `check_growth_after`).
- Not covered here: `relation add` with the old ID (mara-s55h22, after mara-w0xasr); `project fix` rewriting what a stopped rename leaves (VER-FIX-CASES, mara-pvqpse); code markers (mara-yhxq5x).

Checks, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (10 passed, 2 ignored, load about 2), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

- Syntax as in 0.3.0: `item move <id-or-mid> <path> [--line <n>]`, the path relative to the project root, the item before line `n` of the destination as it is now or at its end; `item rename <id-or-mid> <new-id>` (DES-MOVE-AND-RENAME).
- The moved item keeps its bytes, line endings included. Its own relation links and mentions name the same files from the destination (a stale one stays stale: no repair), those to itself the destination. Every relation link and mention to it elsewhere names the destination, a stale one included, since REQ-MOVE-ITEM says every inbound link. Only path bytes change; a link that already resolves keeps them. A link decided by a definition gets its path through the definition, and every reference using it is written as it is (the "each link" form of DES-ITEM-WRITER).
- Other relative links of the moved item (plain files, images) keep their bytes and break on a move to another directory: one sentence in DES-MOVE-AND-RENAME, backlog mara-y9yz9c.
- mara-jnvzxr, simplest option: refuse when any reference of the moved item uses a definition outside `Item::span`, which includes definitions after its last block. Carrying those: backlog mara-h8bc9x.
- A move to where the item is (only blank lines and separators between) writes nothing. No `--line` means the end of the destination, also within the same document, as in 0.3.0.
- Rerun of an interrupted move: exactly two items with the MID, one in the destination and byte-equal to what the move would write, wherever it is; then only the source copy is removed and links rewritten. Otherwise `ambiguous-item`.
- A new destination document must be an input as the last listing shows it: `not-an-input`, or `symbolic-link` for a link on the path. Its missing directories are created by the write procedure.
- Rename: a malformed new ID is `invalid-argument` (exit 2, checked before anything runs); a missing prefix or a taken ID is refused with the code of the diagnostic it would cause (`wrong-id-prefix`, `duplicate-id`); an item without an `ID` row is `missing-row`. It rewrites `ID` rows holding the old ID, mention text that reads exactly the old ID, and every relation link text that is not the new ID. Inside the brackets, the old text written once as it reads is replaced (emphasis stays), else the whole text; a collapsed or shortcut reference becomes `[<new ID>][<label>]`.
- Each file is written once, at the first step that changes it; a rename writes the item's file at the `ID` row step, after every other file with mention text. So a file holding mention text and relation links gets its relation link text before the `ID` row: a stop there leaves stale link text, which validation reports and a rerun completes (VER-INTERRUPTED-EDITS allows it).
- `unreadable-edit` also refuses a link whose path or text bytes Mara cannot locate (for example an entity in its destination), with a diagnostic at the link.
- Results: the item as `get` summarizes it where it is afterwards, `from` (move) or `old_id` (rename), `files` in write order, and the inventory's `diagnostics`.
- Performance harness: a write case learns what it writes from one untimed run (a diff of the tree) and restores it before each run outside the timed span (`check_growth_after`); the counter test waits until the written files are 2 s old so the checks read nothing again. Hub cases: rename `REQ-HUB` to `REQ-HUB-RENAMED`, and move it into the new document `docs/moved/hub.mara.md`.

## Rejected approaches

- Rewriting a moved item's links to the file that now holds each target: it would repair stale links, which only fix may do (REQ-ONE-FIX-OPERATION).
- Rewriting each reference link as an inline link: it changes the author's formatting.
