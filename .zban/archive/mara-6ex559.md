---
title: Item writer and write procedure
status: done
type: feature
parent: mara-6k50w0
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T11:15:01Z
---

Goal: one writer for item syntax and one safe way to put bytes on disk.

Context: DES-ITEM-WRITER in docs/authoring.mara.md; docs/writes.mara.md (REQ-MINIMAL-REWRITE, REQ-REFUSED-EDIT-CHANGES-NOTHING, DES-WRITE-PROCEDURE).

- [x] Writer for anchors, metadata tables and relation links. It never starts a file with `---`: a leading `---` always opens front matter, and an item in it is reported as `item-in-front-matter` and not read (DES-REGIONS in docs/format.mara.md, decided by the maintainer in mara-t6xn9z)
- [x] Write lock on the `.mara` directory; every target checked before the first write; temporary file, second check and rename; re-derivation of the changed files only in a host that keeps state, and none after the renames in a direct CLI call (DES-WRITE-PROCEDURE, ADR-NO-REDERIVATION-IN-CLI)
- [x] The pause points for tests, in debug builds only: `MARA_TEST_PAUSE` names a directory; once per edit, after computing it and before step 2, the writer creates an empty `paused` there, and before each file's second check one holding the file's path; each time it waits until `paused` is gone. Plus the release-build `performance` case that finds no trace of them (DES-WRITE-PROCEDURE, ADR-DEBUG-PAUSE-POINT, decided by the maintainer)
- [x] Tests below the CLI for the writer and the write procedure. Through the real CLI and its pause point, VER-PRESERVED-BYTES, VER-REFUSED-EDITS, VER-INTERRUPTED-EDITS and VER-CONCURRENT-WRITERS (docs/verification-writes.mara.md) and the writer row of VER-GITHUB-RENDERING (docs/verification-documents.mara.md) pass with each write operation, in the issue that ships it

## Summary of Changes

- Item writer, `mara_engine::write` (no I/O): `mid`, `anchor`, `label`, `cell`, `heading`, `row`, `table`, `link`/`relative`, `item`, and splices at parser offsets: `insert`, `remove`, `add_row`, `remove_row`, `set_value`, with `line_ending`. Every splice carries an `Expect`, and `ReadBack::edit` applies a document's splices and reads the result back, parsing each distinct new content of one edit once (review round 1). `label` and `relative` moved here from validation. The CLI's copy of `label` is gone, and validation's bare-ID suggestion now calls `link`.
- Parser: `Row::span`, `Row::value_span` and `Item::table` expose the offsets that edits splice at. `PARSER_VERSION` is 8.
- Write procedure, `mara::write`: `WriteLock::take` (std `File::lock`, i.e. `flock`, on `.mara`; no new dependency), `Basis` (`Absent` or `Contents`), `Basis::read` for files outside the snapshot, `FileWrite`, `apply`, `WriteError`, `new_mid`, and the debug-only pause points. Also `Inputs::basis(path)` and `impl From<WriteError> for Failure` in the CLI.
- How an operation uses it: select the root, then `WriteLock::take`, then `Inputs::new` + `snapshot`, refuse `invalid-project`, then build `Vec<FileWrite>` in write order (`Inputs::basis` for documents, `Basis::Absent` for new files, `Basis::read` for any other file), then each document's splices through one `mara_engine::write::ReadBack` per edit, `read_back.edit(&old, &splices, work)?`, (refused with `unreadable-edit`, `Misread::diagnostic`), then `write::apply(&lock, &files, work)?`.
- Tests below the CLI:
  - Engine writer: every title, cell and link reads back as written, and a project written with odd paths is valid.
  - Insert and remove: separators and front matter, with CRLF, a BOM and no final newline.
  - Row edits keep every other byte.
  - `tests/write.rs`: order, modes and untouched files; refusals leave the tree byte-identical with unchanged mtimes; the 2 s rule's counts (0 when settled, 2 reads and 2 hashes per file otherwise); `Basis::read`; symbolic links; the lock waits; writer and procedure end to end.
  - `tests/write_pauses.rs`: pause order; refusal at the step-2 pause (first, later, only and new file); refusal at the first file's step-3 pause; a stop at a later file's pause; a failed rename.
- Release `performance` case `a_release_build_has_no_pause_points`. Its write half is now an item of mara-w0xasr, which links the procedure into the binary.
- Docs: DES-WRITE-PROCEDURE, DES-ITEM-WRITER, DES-OPERATIONS (`changed-file`), and the `io-error` and `symbolic-link` rows of DES-PROJECT-CONFIGURATION.

Checks, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (10 passed, 2 ignored, load 0.7), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

- The lock is std `File::lock` (`flock`) on `.mara` opened as a directory, waiting while another writer holds it. It needs no dependency, and `flock` behaves the same on macOS (mara-ag4ees). `.mara` is checked with `lstat` and compared by device and inode after opening. A link is refused with `symbolic-link` at line 1 of `.mara/config.toml`.
- A target, or a directory between the root and the target, that is a symbolic link refuses the edit with `symbolic-link`. Nothing is written through or replaces a link. This is no looser than DES-PROJECT-CONFIGURATION.
- A file the edit creates must still be absent at both checks. Its missing directories are created in step 3, just before its temporary file, so a refusal creates nothing.
- Temporary file: `.mara-tmp-<pid>-<n>` in the target's directory, short whatever the target's name and never `*.mara.md` (review round 1, finding 8). It gets the target's permissions after its contents are written, since writing clears set-user-ID and set-group-ID bits (finding 6). It is fsynced before the rename, and the directory is fsynced after the rename and after each directory created. On failure it is removed. When that removal fails, or a directory on its path became a symbolic link, the message says that no file was replaced and names the temporary file instead of saying that nothing was written, and `WriteError::temporary` holds it (finding 1). This follows 0.3.0, which also synced files and directories.
- Decided by the maintainer (review round 1, finding 1), ADR-WRITE-PATHS-AS-USED: paths are checked as they are when the procedure uses them, as for reads; DES-WRITE-PROCEDURE states the window (a directory replaced during a write can redirect the temporary file's rename or removal and leave it behind). Question: "Review of the write procedure (mara-6ex559) found a race. While Mara writes a temporary file, another program renames the directory and puts a symbolic link in its place. Mara's second check then refuses with `symbolic-link`, but its cleanup follows the link and deletes a same-named file in the link's target, outside the project; the real temporary file is left behind. In alpha.1 you ruled the same kind of race out of scope for reads ("a process that can swap files in the project during a load can already write any schema it wants"). What applies to writes?" Chosen option: "Out of scope, document it (Recommended) — Same rule as for reads: paths are checked as they are when Mara uses them. A program that can swap a project directory within the milliseconds of a write, and knows the hidden temporary file's name, can already delete that outside file itself. DES-WRITE-PROCEDURE states the window. No new dependency; a directory moved mid-write can leave one hidden temporary file behind." Rejected: directory handles (needs operating-system calls the standard library lacks, platform-specific, more code on the most safety-critical path); a cheap mitigation that re-checks the path before cleanup (narrows the window without closing it).
- Decided by the maintainer (after review round 1), ADR-READ-BACK-EVERY-EDIT: the read-back's parse is allowed. The cold write row of VER-WORK-COUNTERS now reads "the counts of a cold request plus 1 parse per distinct content written, before the renames" and still allows no parse after them. The row for a write that is a host's first request now says those parses come before the renames and the snapshot build after them. The other warm write rows stay as they are, since a host that keeps state can reuse the read-back's parse. `ReadBack` replaces the free `edit`: it parses each distinct new content of one edit once, and `a_cold_write_counts_one_more_parse_per_distinct_content_written` and `reads_back_each_distinct_content_of_an_edit_once` assert the counts. Question: "To fix a review defect (an inserted item silently swallowed by an unclosed code fence), the writer now parses each document it is about to write, once, and refuses the edit if the result does not read back as intended. That is one extra parse per written document, before anything is renamed. The cold write row of VER-WORK-COUNTERS (Accepted) says "the counts of a cold request and nothing more", so it forbids that parse. Which do you want?" Chosen option: "Allow the parse (Recommended) — The row becomes "the counts of a cold request plus 1 parse per distinct content written, before the renames". Every edit is verified by reading it back, so no edit can silently lose or swallow an item whatever the document looks like. Cost: one parse of each changed document per write; nothing after the renames, as you decided. A daemon can later reuse that parse." Rejected: dropping the guard and teaching each editing function about open code blocks, HTML blocks and comments (any case nobody thought of is silent data loss).
- Decided by the maintainer (after review round 2's re-check), ADR-READ-BACK-EVERY-EDIT and DES-ITEM-WRITER: the read-back exempts only the exact bytes that the splices replace or insert and compares everything else in the document. Question: "Issue mara-6ex559 (item writer) has ended two review rounds with the same defect, so by your rule I stop and ask. Before writing, the writer reads every edit back and must refuse one that changes what text it does not touch links to. Last round's cases are fixed, but the reviewer found two new ones that are accepted: inserting an item between two narrative paragraphs can silently retarget their links, and editing a link reference definition can silently turn plain text elsewhere into a link. How should the issue continue?" Chosen option: "Third round, stricter rule (Recommended) — Same worker. Instead of patching the two cases, the check skips only the exact bytes the edit changes and compares everything else in the document, with no shortcut for an item or narrative that the edit merely touches. A new reviewer re-checks. If that round ends with this defect again, I stop and come back to you."
- Every edit is read back before it is written (review round 1, finding 3, and review round 2, DES-ITEM-WRITER). `ReadBack::edit` parses each distinct new content once and compares the result with the parse the edit was computed from. It refuses with `unreadable-edit` (new in DES-OPERATIONS), naming what would read differently and why, unless:
  - the front matter is the same;
  - (since review round 3, decided by the maintainer below) only the bytes that the splices replace or insert may read differently. Every item whose anchor the edit leaves alone is read where the anchor moved, with its title and end. Every row, every link, image and footnote reference, and every top-level block whose own bytes the edit leaves alone reads as before apart from offsets, in the same item or narrative. Nothing new reads as an item, row, reference or block outside the written bytes;
  - each removal takes exactly one item and only blank lines, spaces and separators around it, and each item the edit changes has the rows its row changes give it;
  - an inserted item goes between blocks and link reference definitions, starts where it was put, ends within its text, and its links that a definition decides use one inside it;
  - no parse problem appears outside an inserted item.
  The old reading comes from the parse the edit was computed from, so the guard costs no second parse. To give it what decides each reading, the parser records every link, image and footnote reference with its resolved destination, title and deciding definition (`Document::references`), the spans of the top-level blocks (`Document::blocks`) and those of the first link reference definitions (`Document::definitions`). PARSER_VERSION is now 10. Chosen because it needs no knowledge of block context and covers the whole family of documents that load without being well formed. Rejected: making `insert` aware of open code, HTML and comment blocks, which would cover one function and one family member.
- New failure code `changed-file` in DES-OPERATIONS. `io-error` (with "cannot read" or "cannot write") and `symbolic-link` also cover writes. The diagnostic is at line 1 of the file concerned, and after the first rename the message lists the files already replaced.
- The pause points read `MARA_TEST_PAUSE` (empty counts as unset). `paused` is written under a staging name and renamed into place, and the writer polls every 5 ms for it to be gone.
- Writer:
  - New lines take the document's first line ending, LF without one. A row added to a table takes the line ending of the table's last row.
  - The built-in key `id` is written `ID`, and `Type` goes in the header row.
  - A title or cell that is empty, holds a line break, or starts or ends with a space or tab is refused.
  - `|` is written `\|`. A heading escapes a trailing `#` run that would close it.
  - Link text escapes ASCII punctuation except `-`. A link path percent-encodes controls, the space and `"%&#?()<>\|:`, and keeps other characters as they are. The `:` is encoded so that the part before it never reads as a URL scheme (review round 1, finding 7). Rejected: prefixing `./` only when needed, which is a second rule.
  - Insert adds `---` separators as needed, reuses an existing separator, and puts nothing before an item at the Markdown start. A closing front matter line without a line ending gets one (finding 5).
  - Remove takes one separator with the item: the one after it when the item comes first. Text before a merged anchor on its line stays as narrative (finding 2), and a line right after the item, such as an anchor without its separator or a `---` after a table, keeps a blank line before it.
  - Removing the header row moves the next row's text into its place and rewrites the delimiter row when its column count differs from that row's cells (finding 4). It returns `None` only when no other row exists. Removing a last row that has no line ending takes the line ending before it, so the file still ends without one.
- A MID uses 80 bits from `/dev/urandom` and the host's time in milliseconds (Linux and macOS are the only platforms).

## Rejected approaches

- A lock crate (`fs2`, `fd-lock`): std's `File::lock` does the same.
- Passing a pause directory as a parameter of `apply`: that is a seam that ADR-DEBUG-PAUSE-POINT rejects, and it would leave a trace in release. The tests set the variable in a one-test binary instead.
- Refusing to remove a header row: `relation remove` would fail on a hand-written table whose header is a relation.

## Review round 1

Fixed in one commit on top of 607f8a7. Each fix has a test that fails without it; I checked this by undoing each fix in turn and running its test.

1. Cleanup message (`crates/mara/src/write.rs`, `Target::replace`, `Refusal::into_error`): the message no longer says nothing was written when the removal is unconfirmed. Test: `write_pauses`, cases "removal refused", "directory gone" and "directory linked". Window and decision: DES-WRITE-PROCEDURE, ADR-WRITE-PATHS-AS-USED.
2. Merged anchor (`write::remove`). Test: `removes_a_merged_anchor_item_and_keeps_the_text_before_it`.
3. Read-back guard (`write::edit`, `read_back`). Tests: `refuses_an_inserted_item_that_would_not_read_back`, `refuses_every_edit_that_does_not_read_back_as_written`.
4. Delimiter on header promotion (`write::remove_row`, `cells`). Test: `removes_a_header_row_whose_columns_differ_from_the_next_row`.
5. Front matter without a final newline (`write::insert`). Test: the new cases of `inserts_an_item_as_its_own_region_and_never_starts_a_file_with_a_break`.
6. Mode set after writing (`Target::replace`). Test: `keeps_special_permission_bits`.
7. `:` encoded (`write::relative`). Test: `writes_relation_links_that_resolve_and_read_as_written`, which now holds colon paths and body mentions.
8. Short temporary names (`temporary_file`). Test: `writes_a_file_whose_name_is_as_long_as_names_go`.

Other members of the family, found by running every editing function on items that load although they are not well formed, and at every line of mixed documents:
- A line right after a removed item now keeps a blank line before it. Test: `removes_an_item_without_joining_the_lines_around_it`.
- A last row without a line ending is now removed without leaving one. Test: `edits_rows_in_place_with_the_table_line_endings`.
- The sweeps stay as tests: `edits_items_that_load_although_they_are_not_well_formed` and `inserts_at_every_line_and_removes_every_item`. Only the removal of a `Type` header row, an insertion after a body with an unclosed fence, and an insertion inside a code block are refused, and all three would lose something.

## Review round 2

Fixed in one commit on top of 2dbc04c. Each fix has a test that fails without it; I checked this by undoing each check in turn and running its test.

1. Untouched relations and mentions (`crates/mara-engine/src/write/read_back.rs`, `compare_item`, `compare_unit`, `cause`): what the edit does not touch must read as before, link targets included. A difference is refused, naming the item or line and the definition responsible ("is in the item being removed", "the edit changes", "would come first and shadow", "would make it a link"). Test: `refuses_edits_that_change_what_other_text_reads_as`, starting with the reviewer's case.
2. Ordinary insertions (`inserted`): an inserted item must start where it was put, end within its text, and use only definitions inside it. Trailing definitions and spaces are allowed and kept. The three refusals now say what differs: not read where written, runs on past its text, or uses an outside definition. Test: `inserts_and_edits_items_with_every_kind_of_body`, with 13 kinds of body (reference definitions inside and trailing, trailing spaces, hard breaks, HTML, lists, code, nested quotes, links, tables, footnotes, a titled definition) in 4 host documents (empty, no final newline, with items, CRLF), each inserted at the start and the end, then edited and removed.
3. Verbatim quotations: the write-race decision now quotes the question and the chosen option exactly as sent. The guard-parse decision already did.

Other members of the family:
- An insertion that would split a block or a link reference definition (a paragraph, lazy line, list, table, quote, code block, HTML block, setext heading or definition title) is refused (`insertion_points`).
- New narrative that appears where no touched narrative was, such as a later duplicate definition's title split off, is refused.
- `inserts_at_every_line_and_removes_every_item` marks each line of its documents as starting or continuing a block. It checks that exactly the continuing lines are refused and that every item can be removed, in LF and in CRLF.
- Cost: still one parse per distinct new content. Lookups are by binary search, so the guard is about 1.5 times a parse at every size from 4,000 to 64,000 rows in one item.

## Review round 3

Fixed in one commit on top of 71f5096. Each fix has a test that fails without it; I checked this by undoing each part of the rule in turn and running the tests.

The rule changed as the maintainer decided (see `## Decisions made`). Only the exact bytes that the splices replace or insert may read differently. `crates/mara-engine/src/write/read_back.rs` was rewritten around four kinds of element, compared at mapped offsets across the whole document:
- items, by anchor, with title and end;
- metadata rows;
- references: every link, image and footnote reference;
- top-level blocks.

An element is exempt only when its own bytes change, and anything new outside the written bytes is refused. The early return for items with `Expect::Bytes` splices (D2) is gone. So are the unit-level touched skip (D1) and the narrative-unit comparison, which the block comparison replaces. No other place exempts more than the changed bytes.

- D1 (`references`): unchanged references are compared one by one wherever they are. Test: `refuses_any_change_to_what_unchanged_bytes_read_as`, cases D1 (shadowing, a definition that turns plain text into links, and an insertion together with the removal of the item that holds the definition).
- D2 (`references`, `rows`, `items`): nothing new may read as a reference, row or item outside the written bytes. Test: the same, case D2.
- D3: line numbers come from an index built once. Ownership, rows, references, blocks and problem lines are found by binary search, and the unit path is gone. The writer's `insert` and `remove` also scanned the whole document on each call (11.3 s and 3.8 s for 1,500 calls on one document); they now scan only the lines around their position (`Around`). Test: `reads_back_many_splices_in_time_proportional_to_the_document`, with 1,500 removals in one edit and 1,500 insertions that split narrative units, each against two parses: the guard costs 0.1 and 0.9 parses, and the old quadratic line lookup made it 40.
- M1: ADR-WRITE-PATHS-AS-USED already says that the maintainer chose it. No change.

Trying to break the rule, also covered by `refuses_any_change_to_what_unchanged_bytes_read_as`:
- a narrative heading's link losing its definition;
- an image, and a link to a web page, whose definition changes;
- a footnote that an inserted item shadows;
- two paragraphs joined by changing only the blank line between them;
- narrative joining an item;
- an anchor written before an incomplete item, which would turn unchanged rows into an item's.

`accepts_the_edits_that_operations_make` checks that what the operations of this slice write still reads back:
- rename of mention text, then a relation link;
- move rewriting a definition together with the links that use it;
- fix rewriting a setext heading as an ATX heading;
- an anchor written with its whole item;
- deleting an item that holds the first definition of a label that a later definition then takes over. Accepting this one needed link reference definitions kept apart from blocks (`Document::definitions`).

The two sweeps now also cover lone-CR documents.

Consequence for the operations: an edit that means to change what other text reads as must include that text in its splices. A move retargets links that a shared definition decides by rewriting the definition together with those links, or each link. Fix adds a missing anchor with a splice that holds the whole item.
