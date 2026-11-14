---
title: Parse Mara documents
status: done
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-0ztabc
  - mara-csmk0c
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T14:53:32Z
---

Goal: one pass per file turns a `*.mara.md` document into items, metadata, links and diagnostics.

Context: docs/format.mara.md (DES-REGIONS, DES-ITEM-STRUCTURE, DES-METADATA-TABLE); docs/identity.mara.md (DES-MID-ANCHOR, DES-ITEM-IDS, DES-RELATION-LINKS); DES-SNAPSHOT in docs/engine.mara.md for the single `pulldown-cmark` pass. Verified end to end, through `project validate` (mara-000f9q), by VER-GITHUB-RENDERING and VER-SEEDED-PROBLEMS in docs/verification-documents.mara.md.

- [x] Decide whether YAML front matter is accepted: skipped at the start of a file (DES-REGIONS)
- [x] Regions, items and incomplete items
- [x] Metadata table with label matching
- [x] Relation links and mentions
- [x] Diagnostics for misplaced anchors, setext headings and malformed rows
- [x] The documents in `docs/` parse without diagnostics

## Summary of Changes

- Engine (`crates/mara-engine`): `document::parse(bytes, &mut WorkCounts) -> Document`, one `pulldown-cmark` 0.13.6 pass (no default features: `bitflags`, `memchr`, `unicase`; builds for `wasm32-unknown-unknown`). `Document` holds the items (MID, title, metadata rows with key, schema name, value, whole-cell link and line, mentions, byte spans, anchor line), the narrative mentions and the problems; `Document::diagnostics(path)` yields them as `Diagnostic`s. Also `label_name` and `is_mid`. Each call counts one parse. `diagnostic.rs` is byte-identical to b8f8d27 (mara-b52dg3).
- Tests: 15 engine unit tests (the documented example of docs/format.mara.md, `***`/`___` breaks and `---` in code, front matter with BOM and CRLF, every incomplete-item form, misplaced and merged anchors, setext headings, every malformed-row form, label matching, relation links and mentions, empty body, the parse count, lines with LF/CRLF/CR, non-UTF-8 input, no panic on every prefix, a 4 MB line, 50,000 nested quotes, 20,000 nested lists and 100,000 brackets); `crates/mara/tests/documents.rs` parses every `docs/*.mara.md` read-only and requires no diagnostic and an item at every anchor line outside code; `corpus::tests::writes_the_current_item_format` parses every declared document of the 1×, 4× and 16× corpora: no diagnostic, the generated item count, one parse each.
- Docs: decisions written into DES-REGIONS, DES-ITEM-STRUCTURE, DES-METADATA-TABLE, DES-MID-ANCHOR, DES-RELATION-LINKS, DES-INPUTS-AND-KEYS, DES-SCHEMA-CHECKS and DES-PERFORMANCE-HARNESS. No document in `docs/` needed a fix.
- mara-000f9q, mara-bmc9gc and mara-8d6cnw name what is left for them (scaling of the parse through their operations, narrative units from the same pass, the parser version in the parsed-document key).

Checks: the six local checks of AGENTS.md exit 0. Outside the repository: 23,000 randomly edited copies of the `docs/` documents parsed without a panic (release and debug), and parse time grew linearly from 2,000 to 16,000 items in one document (6 ms to 44 ms).

## Decisions made

- API: the parse takes bytes only, and its problems take a path when reported (`Document::diagnostics(path)`), because the parsed-document key is parser version + file hash. Rejected: a path parameter, which would make the key wrong for a moved or duplicated file. (DES-INPUTS-AND-KEYS)
- Parsing reports the Document structure failures and malformed rows; required, repeated and undeclared rows, values, identity and links stay with validation. (DES-SCHEMA-CHECKS)
- Codes: `invalid-utf8`, `html-end-tag-case`, `item-in-front-matter`, `missing-separator`, `merged-anchor`, `setext-heading`, `incomplete-item`, `empty-body`, `malformed-row`. All are errors. (DES-REGIONS, DES-ITEM-STRUCTURE, DES-METADATA-TABLE, DES-MID-ANCHOR)
- A file that is not UTF-8 gives one error at the line of the first invalid byte and no items, as the configuration and schema do. Rejected: lossy decoding, which shifts byte spans. (DES-REGIONS)
- Lone CR (third review): `pulldown-cmark` 0.13.6 ends paragraphs, headings, tables, lists, quotes and thematic breaks at a lone CR, but not fenced code, indented code or HTML blocks, so lone-CR documents were misread there. The parser therefore reads each lone CR as LF before its one Markdown pass, in a copy made only when the text holds one; one byte replaces one, so spans and lines stay those of the file, and the result equals that of the LF form. Rejected: patching each recovery that sees the misread blocks. (DES-REGIONS)
- A UTF-8 byte order mark is skipped, and a line ends at LF, CRLF or a lone CR, in front matter too. Front matter is recognized by the parser itself, as the Accepted sentence has it: a `---` first line, any lines, the next `---` line. Rejected: pulldown-cmark's metadata blocks, which also close at `...`; a non-blank second line, which narrowed the Accepted sentence (review). How a leading `---` is read was decided by the maintainer (below). (DES-REGIONS)
- An item in front matter, carrying out the maintainer's decision: code `item-in-front-matter`, an error at the line of each MID anchor in the block, every anchor reported, since each is an item the user loses and must move. An anchor counts when a line of the block, without the spaces and tabs around it and its LF, CRLF or CR ending, is exactly `<a id="<MID>"></a>` with a MID, as DES-MID-ANCHOR spells it; the block is scanned line by line once, not parsed as Markdown, and stays skipped. Rejected: one diagnostic per block, which would not point at every lost item. (DES-REGIONS)
- A thematic break inside a list or block quote ends its region after the top-level block that holds it. Rejected: splitting the list or quote between two regions. (DES-REGIONS)
- A MID anchor paragraph that is not first in its region is reported and starts a new region, so its item loads. Rejected: leaving it inside the previous item, which hides that item and its own problems and breaks every link to it. (DES-REGIONS)
- Only the exact spelling `<a id="<MID>"></a>` is an anchor, a misplaced one included: an opening tag alone is neither an anchor nor reported (second review). A MID anchor in a paragraph with other content, inside another block, or not followed by a blank line is `merged-anchor`; inside an HTML block only a line that is exactly the anchor counts (decided by the maintainer, below), found by the same line function as in front matter. An anchor on the last line of a file has no blank line after it and is reported: DES-MID-ANCHOR asks for a blank line after the anchor, and the reviewer read it so. An item whose anchor lacks only the blank line loads, as GitHub still renders the anchor before the heading (VER-GITHUB-RENDERING). The check reads the source after the anchor, so it holds when no block follows. A paragraph of inline HTML only, such as a lowercase MID anchor, before a heading and `Type` table makes an incomplete item instead of silent narrative. (DES-MID-ANCHOR, DES-ITEM-STRUCTURE)
- One `Type`-row rule everywhere a region is classified: a row whose first two cells are `Type` and a value, with the excess-cell recovery. Every problem of a region that is or is meant as an item is reported once: anchor form, malformed rows and an empty body also for an incomplete item; without a heading, the table after the block in its place is still the metadata table. A region whose first paragraph holds exactly one merged MID anchor is meant as an item, also without a heading or `Type` row; it loads when it has both, its span and line start at the anchor, and links before the anchor are narrative. A MID anchor merged into a heading, a list, a quote, an HTML block, or one of several in the first paragraph makes no item and is reported only as `merged-anchor`, never also as a missing anchor. Rejected: recovering items from those, which made identity, span and double reports harder to keep right; reporting without loading is what REQ-REPORT-EVERY-PROBLEM asks. (DES-ITEM-STRUCTURE)
- An uppercase end tag, carrying out the maintainer's decision: code `html-end-tag-case`, an error at the line of the first end tag of the block's own kind in another case than lowercase, once per block. A lowercase end tag on the same line ends the block for `pulldown-cmark` too and is no error. After it, no anchor line in the rest of that block is reported, a simple per-block flag. Other follow-on effects, such as a `missing-separator` after a swallowed separator once the block ends, are left: they come after the error in line order, and suppressing them would mean re-reading the swallowed text. Another kind's end tag (`<pre>` … `</script>`) is not reported: the decision covers a block's own end tag; mara-zq6awt records it. (DES-REGIONS)
- A setext heading is reported at its underline; in the title position it is read as the heading. An item without a body is reported and still loads. (DES-REGIONS, DES-ITEM-STRUCTURE)
- A row without a key or value is reported and not read; a row with more than two cells is reported and keeps its first two, so a stray `|` never makes an item vanish (REQ-INVALID-ROWS-REPORTED). A value is the cell's source, trimmed, with `\|` as `|`. (DES-METADATA-TABLE)
- Labels match names after ASCII lowercasing and writing spaces as `_`. (DES-METADATA-TABLE)
- A row's value is a link only when one link is the whole cell, in any form: inline, full, collapsed or shortcut reference, or autolink. A link's span covers all of its source; the parser's span of a collapsed reference lacks the closing `[]`, which the reader adds. A mention is any link outside a metadata table, the item heading included, whose fragment is a MID and whose destination has no URL scheme. (DES-RELATION-LINKS)
- GitHub's extensions are on: tables, footnotes, strikethrough, task lists, so `[^1]: …` is a footnote and not a link definition, as on GitHub.
- Parsing cost: the parser adds no harness case; it counts one parse per call, the corpus test asserts one parse per generated document, and the operations that parse (mara-000f9q, mara-bmc9gc) assert and time it, as their VER-SCALING items now say. (DES-PERFORMANCE-HARNESS)
- The parser version of the cache key is left to mara-8d6cnw, which computes the key; its item says so.

## Review fixes

Six review findings fixed in a second commit: front matter follows the Accepted sentence again and reads LF, CRLF and CR; collapsed reference links have their whole span and count as whole-cell values; MID anchors in HTML blocks are reported; a missing blank line after an anchor is reported; incomplete items and merged first anchors no longer hide malformed rows or an empty body. Filed mara-zq6awt (pulldown-cmark ends a `script` block only at a lowercase end tag). Checks: the six local checks exit 0; 23,000 random edits of `docs/` and of 97 generated documents parsed without a panic; parse time stays linear (7 ms at 2,000 items, 50 ms at 16,000).

## Decided by the maintainer

The question: "Parser (mara-t6xn9z): DES-REGIONS says a front matter block at the start of a file is "a --- line, YAML lines, a --- line" and is skipped. But a file may also start with a --- separator. Read literally, `---`, blank line, item A, `---`, item B skips item A as front matter with no diagnostic. How should a leading --- be read?"

> A: keep rule, guard items — The worker's recommendation. Keep the Accepted sentence: a leading --- always starts front matter. Add that writers never start a file with ---, and report a skipped block that contains a MID anchor as a new error, so no item is lost silently. A narrative-only first region after a leading --- is still skipped without a diagnostic.

Carried out in a third commit: DES-REGIONS states the writer rule, the accepted consequence and `item-in-front-matter`; the open question is removed from docs/format.mara.md; no document in `docs/` and nothing the corpus generator writes starts with `---`; mara-6ex559 (the item writer) names the rule. VER-SEEDED-PROBLEMS lists no codes, so it is unchanged.

## Decided by the maintainer (second review)

How anchors inside HTML blocks are found:

> A: exact line (Recommended) — Inside an HTML block, any line that is exactly the anchor (the same line rule you approved for front matter) is reported as merged-anchor. No HTML tokenizer in the engine. An anchor line inside an HTML comment or a script block is then reported too, and an anchor sharing a line with other HTML is not.

The dependency's uppercase end tag (mara-zq6awt):

> B: dedicated error now — The parser detects an HTML block of those four kinds whose end tag is not lowercase and reports a specific error telling the user to lowercase it. Small and honest, but it makes a document GitHub renders correctly invalid in Mara until the dependency is fixed.

Carried out in a fourth commit, with second-review findings 3–6: recovery needs the complete anchor, a merged first anchor reports every problem of its region and gives the item the anchor's line and span, and an anchor at the end of a file needs a blank line too. mara-zq6awt stays open at normal priority for the upstream report and the removal of `html-end-tag-case`.
