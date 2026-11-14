---
title: "Read operations: search, get, relation list, item list, schema inspection"
status: done
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-8d6cnw
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T19:22:33Z
---

Goal: bounded reads over the snapshot from the CLI.

Context: docs/retrieval.mara.md (all requirements, DES-NARRATIVE-UNITS, DES-PAGING); REQ-SCHEMA-INSPECTION in docs/project.mara.md.

- [x] Decide how `get` pages a large node: consecutive portions (DES-PAGING)
- [x] `search` with filters, ranking and typo tolerance
- [x] `get`, `relation list`, `item list`, `schema get` and `schema list`
- [x] Paging with cursors that carry the snapshot key
- [x] VER-SCALING covers each operation, including its cold parse of every document: the parser has no case of its own (DES-PERFORMANCE-HARNESS, mara-t6xn9z). Narrative units and search text come from the parser's one pass (DES-SNAPSHOT): `document::parse` returns items, rows and links only, so extend it rather than parse again
- [x] The cold rows of VER-WORK-COUNTERS (docs/verification.mara.md) pass through the real CLI for each read operation: a cold request (two documents with the same contents included: 1 parse per distinct content), `.git` and undeclared files, an operation that reads no code (moved here from mara-8d6cnw by the maintainer, mara-63dn2d)
- [x] VER-SEARCH-CASES, VER-NODE-READS, VER-ITEM-LIST, VER-CURSOR-WALK, VER-PAGE-LIMITS and VER-SCHEMA-INSPECTION pass (docs/verification-retrieval.mara.md)
- [x] VER-ONE-HOP passes except its code-link case, which waits for code links (mara-d005q9)
- [x] VER-MID-RESOLUTION and the `get` case of VER-INVALID-ROWS pass, and VER-NESTED-PROJECTS and VER-UNCHANGED-SOURCES pass for the read operations (docs/verification-documents.mara.md)
- [x] The read operations select and load the project with `mara::project::select` and `load` (crates/mara/src/project.rs, mara-b52dg3), which only host tests have exercised. Real-CLI tests cover selection from inside a nested project and with `--project`, `no-project`, `not-a-project`, `legacy-project` and `symbolic-link` (codes in DES-PROJECT-CONFIGURATION); each read operation refuses a project whose configuration or schema has an error and reports those diagnostics (mara-zv9d69); `schema list` and `schema get` return every declaration of DES-SCHEMA-MODEL as loaded: per-source targets, the `code` source, `same_type`, `acyclic`, `required` and `repeatable`

## Summary of Changes

Read operations on the CLI skeleton of the validation slice: `search`, `get`, `relation list`, `item list`, `schema list` and `schema get` (`crates/mara-engine/src/read/`, `crates/mara/src/cli/read.rs`). The parser's one pass yields narrative units, each word's zone and first source offset; the snapshot keeps units and builds a symmetric-deletion vocabulary index on the first typo-tolerant search. Every read refuses a project whose configuration or schema has an error, pages within 65,536 bytes with cursors keyed by the layer it reads, and carries the inventory's diagnostics. Real-binary tests in `crates/mara/tests/reads/`; six harness cases plus two growing-vocabulary scaling cases in `crates/mara/tests/performance/`.

After review: every page fits even with many inventory diagnostics; document-read cursors cover those diagnostics; typo candidates come from the vocabulary index; nested headings rank as headings; the narrative before a merged anchor holds only its own text; excerpts come from the index's words and offsets; `relation list` reads whole documents. Rebased onto the integrated validation (5914428): path filters use `project_path`, place lists use `validation::list`.

Third round: a query word of more than 18 characters matches only exactly, and every other query word finds its candidates through index lookups alone; metadata values keep source marks, so excerpts start near their word in every zone; the text after a merged anchor starts its item's body for search, excerpts and `get`; a document of more than 2³² − 1 bytes is refused before it is parsed.

Fourth round: every source position of a word comes from the parser's own source ranges, a code span's text from its source lines, and the end of a merged anchor from the anchor itself; no search for rendered text and no fallback position remain, and an excerpt is the whole line that holds its word.

Fifth round: the text indexed, and a metadata value's `Row.text`, are again what the parser rendered; each rendered piece is aligned with its source range by CommonMark's rules, and one it cannot align points at the start of that range. No read operation panics on any input: an excerpt of a node with no text is empty. A test compares indexed words, offsets, excerpts and `Row.text` with the parser's own events for every construct in every zone under LF, CRLF and lone CR, and a seeded fuzz test reads random documents.

Sixth round: a word in a code span over several lines has the position of the code span's start, and its excerpt is the line where the span starts; the alignment across line endings, continuation-line prefixes and the CRLF-as-two case is removed, while a code span on one line and every other construct keep exact positions.

### Decided by the maintainer

- Schema cursors: "A: when schema changes (Recommended) — The worker's recommendation, already implemented. A schema read's cursor goes stale when the configuration or the schema changes, not after a document edit. The VER-CURSOR-WALK row is amended to "after an edit of an input the result reads". A schema read keeps reading 2 files." The row of VER-CURSOR-WALK is amended.
- Search zones: "Accept five zones (Recommended) — Headings are their own search zone and DES-SNAPSHOT says five. The reviewer still checks that the ranking matches REQ-SEARCH-RANKING."
- How the issue proceeds: "Third round, same worker (Recommended) — The same worker fixes the remaining points with its context intact. The fixes are concrete: keep source positions inside metadata values; make the text after a merged anchor searchable as part of its item; reject or correctly handle offsets beyond 4 GiB; and the long-word point as you decide in the next question. A new reviewer re-checks."
- Long words: "Exact match only above 18 (Recommended) — Typo tolerance applies to query words of up to 18 characters; a longer word must match exactly. The requirement is narrowed by one sentence, the slow path and its code disappear, and the follow-up issue mara-y45s0v is closed as settled." REQ-TYPO-TOLERANT-SEARCH is narrowed; mara-y45s0v is cancelled as settled.
- Hash collisions in the typo index: "Out of scope (Recommended) — A project whose words are crafted to collide slows only its own searches, and results stay correct. DES-SNAPSHOT states the limit: a query word's work does not grow with the vocabulary unless words are constructed to share a hash. No code change. The same reasoning as your decision on the symlink race." DES-SNAPSHOT states the limit; an engine test pins that colliding words still match by the rule.
- How the issue proceeds after the third review: "Fourth round, narrow (Recommended) — The same worker fixes this excerpt bug (and the collisions, if you chose to fix them). A new reviewer re-checks only that commit. The branch then rebases onto the scaling-test fix and is integrated when its checks pass."
- How the issue proceeds after the fourth review: "Fifth round, same worker (Recommended) — The worker repairs both regressions: no assertion that a node has text, and indexed text is always what the Markdown parser rendered, with only the offsets mapped to the source. It adds a test that compares indexed text with the parser's rendering for every construct. A new reviewer re-checks only that commit."
- How the offset mapping ends, after the fifth review: "Simplify the rule (Recommended) — Decide that for a word inside a code span that runs over several lines, the excerpt is the line where the code span starts. DES-SEARCH-REQUESTS says so in one sentence, the character-by-character mapping across lines is removed, and single-line cases keep exact positions. Smallest code, no further corners of this kind; the cost is that such an excerpt may not contain the word."

## Decisions made

- Typo tolerance counts optimal string alignment edits over characters (REQ-TYPO-TOLERANT-SEARCH).
- An `item list` summary holds kind, path, line, ID, MID, type and title (REQ-LIST-ITEMS).
- A page limit outside 1–100 is refused before anything runs; `get` pages by bytes alone (REQ-PAGE-LIMITS, VER-PAGE-LIMITS, both Draft).
- Every region that is no item, an incomplete item's included, is narrative; text before a merged anchor is a unit, and the text after the anchor in its paragraph starts the item's body for search and `get`; a line that no node holds names nothing, and a line that a unit and an item share names the one that starts first (DES-NARRATIVE-UNITS).
- Five search zones, a heading being any heading in an item's body, nested ones included, or a unit's; each posting keeps its word's first source offset, of the parser's rendered text, mapped to the source by CommonMark's rules or to the start of the source range it cannot be mapped through, a code span over several lines being such a range (DES-SNAPSHOT, DES-SEARCH-REQUESTS). `PARSER_VERSION` 7, `SNAPSHOT_VERSION` 2.
- The vocabulary index: symmetric deletion over the words of 3 to 8 characters with one deletion and of 6 to 20 with two, the words within reach of query words of 4 to 7 and 8 to 18 characters; built on the first typo-tolerant search and kept with the snapshot, entries of 8 bytes, each deletion string hashed in constant time from prefix and suffix sums; a vocabulary built to share a hash slows only its own searches (DES-SNAPSHOT).
- A document of more than 4,294,967,295 bytes is `document-too-large` and is not parsed, so every source offset fits 32 bits (DES-REGIONS).
- Filters, the query, ranking within a group, and excerpts from the index's words and offsets (DES-SEARCH-REQUESTS).
- Result formats, node addressing, `relation list` of items, units and whole documents, links to a repeated or missing MID (DES-READ-RESULTS).
- A read of documents shows the first inventory diagnostics that fit in 8,192 bytes and `diagnostics_total`; the human form points to `mara project validate` for the rest (DES-READ-RESULTS).
- A result key covers the layer read, the request without page size and cursor, and the inventory's diagnostics; `get`'s offset counts row values, then content; a cut row value is `partial` (DES-PAGING).
- Failure codes `invalid-project`, `unknown-node`, `unknown-declaration` and `invalid-utf8` join DES-OPERATIONS.

Snapshot build at 16× and 64× (median of three alternating rounds against 1fa35f4, release): build 22.9 → 25.0 ms and 99.4 → 116.5 ms; parse 40.9 → 45.1 ms and 166 → 183 ms; the vocabulary index, built on first use, 0.5 ms and 2.3 ms.

Checks: the six local checks of AGENTS.md at the final head, and the 64× cases.
