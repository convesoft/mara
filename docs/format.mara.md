# Item format

```markdown
# Authentication

How users sign in.

---

<a id="01M1PXP2KGVW5ZF2JGP9K4XE9B"></a>

## Phishing-resistant authentication

| Type      | Requirement |
| --------- | ----------- |
| ID        | REQ-PHISHING-RESISTANT-AUTH |
| Status    | Accepted |
| Satisfies | [NEED-SECURE-SIGN-IN](../needs.mara.md#01M1PXQ0R8T4D2VW6YH3KJ5N7A) |

Users must authenticate using phishing-resistant MFA.

### Acceptance criteria

- Passkeys must be supported.

---

<a id="01M1PXP3KGVW5ZF2JGP9K4XE9C"></a>

## Use WebAuthn

| Type | Decision |
| --- | --- |
| ID | ADR-USE-WEBAUTHN |
| Status | Accepted |
| Justifies | [REQ-PHISHING-RESISTANT-AUTH](#01M1PXP2KGVW5ZF2JGP9K4XE9B) |

WebAuthn provides phishing-resistant authentication.

---

Closing narrative that belongs to no item.
```

## Requirements

---

<a id="01M7BF6GYWE2DT7WSXQH2QPNPA"></a>

### A Mara document is its own published form

| Type | Requirement |
| --- | --- |
| ID | REQ-SOURCE-IS-PUBLISHED-FORM |
| Status | Accepted |
| Satisfies | [NEED-READABLE-SOURCE](product.mara.md#01M7BF6H5VABHYDECYVCQ2A9Y8) |

A Mara document is GitHub-flavoured Markdown that GitHub renders as documentation, with every relation link navigating to its target item. Mara has no export, compilation or second publication format.

---

## Design

---

<a id="01M7BF6GZ5V5HWV00MDZN68Z06"></a>

### Documents and regions

| Type | Design |
| --- | --- |
| ID | DES-REGIONS |
| Status | Accepted |
| Satisfies | [REQ-SOURCE-IS-PUBLISHED-FORM](#01M7BF6GYWE2DT7WSXQH2QPNPA) |

- Mara documents are `*.mara.md` files matched by the project's include patterns. Other `.md` files are never scanned.
- A file that is not UTF-8 is an error (`invalid-utf8`), and none of its items is read.
- A UTF-8 byte order mark at the very start of a file is skipped. A line ends at LF, CRLF or a lone CR. `pulldown-cmark` does not end fenced code, indented code or an HTML block at a lone CR, so the parser reads each lone CR as LF before its one Markdown pass; one byte replaces one, so offsets and lines stay those of the file.
- A YAML front matter block at the very start of a file, after any byte order mark (a `---` line, YAML lines, a `---` line), is skipped. It is neither a region boundary nor narrative, and writes preserve it.
- Writers never start a file with `---`, so a leading `---` always opens front matter, and a narrative-only first region after one is read as front matter. Each line of the block that is `<a id="<MID>"></a>` with only spaces or tabs around it is an error at that line (`item-in-front-matter`); the block is still skipped.
- An HTML block that starts with `<script`, `<pre`, `<style` or `<textarea` and holds its own end tag in another case than lowercase, such as `</SCRIPT>`, is an error at that end tag (`html-end-tag-case`). `pulldown-cmark` ends such a block only at the lowercase end tag and reads what follows as HTML, while CommonMark and GitHub match the end tag in any case. No anchor line in the rest of that block is reported.
- Every thematic break (`---`, `***`, `___`) outside code is a region boundary. Start and end of file are implicit boundaries. A thematic break inside a list or block quote ends its region after the top-level block that holds it.
- A region whose first block is a MID anchor is an item. Any other region is ordinary Markdown.
- An item ends only at a thematic break or end of file. Headings never end an item, items contain no thematic breaks, and items do not nest.
- A MID anchor that is not the first block of its region is an error (`missing-separator`, or `merged-anchor` when it is not a paragraph of its own). Reading starts a new region at such an anchor paragraph, and its item loads.
- A setext heading (text directly above `---` or `===`) is an error (`setext-heading`), reported at its underline. Writers put a blank line on both sides of every `---`.

---

<a id="01M7BF6GZDTNNW0Z0MX7FEG6ZF"></a>

### Item structure

| Type | Design |
| --- | --- |
| ID | DES-ITEM-STRUCTURE |
| Status | Accepted |
| Satisfies | [REQ-SOURCE-IS-PUBLISHED-FORM](#01M7BF6GYWE2DT7WSXQH2QPNPA) |

An item region holds, in order:

1. The MID anchor ([DES-MID-ANCHOR](identity.mara.md#01M7BF6H1MB5RR3RS43B3CJHAR)).
2. An ATX heading: the title and nothing else. IDs are never extracted from heading text. A setext heading in its place is reported and read as the heading.
3. The metadata table ([DES-METADATA-TABLE](#01M7BF6GZNS6VM3T94VS9A5HTD)).
4. The body: all remaining blocks, any Markdown. Every type requires a non-empty body; an item without one is an error (`empty-body`) and still loads.

- Every item has `Type`, `ID` and `Status` rows.
- Anchor, heading and `Type` row make a region an item. Markdown without them is never an item.
- An incomplete item is reported and kept in diagnostics (`incomplete-item`). It is either a region that starts with an anchor but lacks the heading or the `Type` row, or a region that starts with a heading followed by a table with a `Type` row but has no anchor. A paragraph of only inline HTML that is not a MID anchor, such as an anchor with a lowercase MID, before that heading counts as no anchor.
- A `Type` row is a row whose first two cells are `Type` and a value, read as [DES-METADATA-TABLE](#01M7BF6GZNS6VM3T94VS9A5HTD) reads rows, wherever a region is classified. Without a heading, the table after the block in the heading's place is the metadata table.
- Each problem of a region that is or is meant as an item is reported once: the anchor's form, malformed rows and an empty body also for an incomplete item.
- A region whose first paragraph holds exactly one MID anchor merged with other content is meant as an item. It loads when it has a heading and a `Type` row, and starts at that anchor; text before the anchor is narrative. A MID anchor merged anywhere else makes no item and is reported only as `merged-anchor`.
- Fix adds the missing anchor ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)). An anchor without a heading or a `Type` row stays reported until the author completes the item.

---

<a id="01M7BF6GZNS6VM3T94VS9A5HTD"></a>

### Metadata table

| Type | Design |
| --- | --- |
| ID | DES-METADATA-TABLE |
| Status | Accepted |
| Decomposes | [DES-ITEM-STRUCTURE](#01M7BF6GZDTNNW0Z0MX7FEG6ZF) |

- The first block after the heading is a two-column GFM table in which every row, the header row included, is `Key | Value`.
- The header row holds the first property; the writer puts `Type` there. Row order is otherwise free.
- A multi-valued property repeats its key row, one value per row. Values are never comma-separated or nested. A row that repeats the key and value of another row is an error.
- `|` inside a value is written `\|`. Column padding is optional. A value is read as written, without surrounding spaces and with `\|` as `|`.
- The table ends at the first blank line. A non-blank line directly after the table is a malformed row and an error (`malformed-row`).
- So is a row without a key or a value, which is not read, and a row of more than two cells, whose first two cells are read as key and value.
- A key the schema does not define for the item's type is an error, even when another type defines it. A value outside a field's allowed set is an error. There is no "allow unknown keys" setting: a new field is declared in the schema.
- Keys and enum values are written as labels derived from schema names: `_` becomes a space and the first letter is capitalized (`derives_from` → `Derives from`, `almost-certain` → `Almost-certain`, type `requirement` → `Requirement`). Reading matches labels case-insensitively: a label matches the name it equals once its ASCII letters are lowercase and its spaces are `_`. Writing uses the derived label.
