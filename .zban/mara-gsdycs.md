---
title: "Converter: 0.3.0 constructs outside realistic projects"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-wjy3sh
created_at: 2026-11-15T15:54:14Z
---

Goal: `project convert` handles the 0.3.0 constructs below as 0.3.0 read them, or reports each one, instead of converting them silently in another way.

Context: DES-CONVERSION-MAPPING in docs/migration.mara.md; code in `crates/mara/src/convert/` (`scan.rs` finds blocks, mentions and Markdown contexts; `documents.rs` converts them; `schema.rs` the schema). Found while building the converter (mara-wjy3sh). None of these occurs in the 0.3.0 corpus (`git show v0.3.0:docs/`), in the 0.3.0 templates, or in the fixtures of VER-CONVERSION, so they were left out of that issue by the maintainer's rule that only realistic setups matter now.

- [ ] A `:::mara` line inside an HTML block or HTML comment, or inside a block quote or list item, is read as no opening line; the scanner tracks fenced code only. Decide whether 0.3.0 opened a block there and match it
- [ ] An unclosed fenced code block in a body runs to the end of the file, so every later block of the document joins that body (0.3.0 behaved the same way, as far as the corpus shows); report it instead of converting it
- [ ] A setext heading or thematic break inside a list item or block quote of a body is kept as it is; only those that start a line are rewritten, so the item can still end early or report `setext-heading`
- [ ] Report lines count LF only, so in a document with lone CR line endings the lines of its entries are wrong
- [ ] YAML front matter in a 0.3.0 document reads as a thematic break and a setext heading when the converter picks heading levels, so its items' headings come one level deeper than intended
- [ ] A 0.3.0 `status` field of type `string` contributes no status, so its items' statuses are not in the converted status list; a relation with both `code_source` and `same_flavour` loses its code source

From the review of mara-wjy3sh, rated exotic or sent to the backlog by the orchestrator:

- [ ] A YAML merge key `<<` in the 0.3.0 schema is not applied, so the properties it would bring in (for example `required` and `repeatable` of a field written `{<<: *text}`) are left out. Each merge key is reported with what to do, and DES-CONVERSION-MAPPING states the limit. Apply merges as 0.3.0 (`serde-saphyr`) did
- [ ] A mention inside a Markdown link label, such as `[details of [[REQ-B]]](https://example.com)`, becomes nested link syntax and breaks the outer link without a report entry (`documents.rs`, where tokens become links). Keep such a token as text and report it, or write a link that does not nest
