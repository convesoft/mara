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
- Every thematic break (`---`, `***`, `___`) outside code is a region boundary. Start and end of file are implicit boundaries.
- A region whose first block is a MID anchor is an item. Any other region is ordinary Markdown.
- An item ends only at a thematic break or end of file. Headings never end an item, items contain no thematic breaks, and items do not nest.
- A MID anchor that is not the first block of its region is an error (missing separator).
- A setext heading (text directly above `---` or `===`) is an error. Writers put a blank line on both sides of every `---`.

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
2. An ATX heading: the title and nothing else. IDs are never extracted from heading text.
3. The metadata table ([DES-METADATA-TABLE](#01M7BF6GZNS6VM3T94VS9A5HTD)).
4. The body: all remaining blocks, any Markdown. Every type requires a non-empty body.

- Every item has `Type`, `ID` and `Status` rows.
- Anchor, heading and `Type` row make a region an item. Markdown without them is never an item.
- An incomplete item is reported, kept in diagnostics and repaired by fix ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)). It is either a region that starts with an anchor but lacks the heading or the `Type` row, or a region that starts with a heading followed by a table with a `Type` row but has no anchor.

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
- A multi-valued property repeats its key row, one value per row. Values are never comma-separated or nested.
- `|` inside a value is written `\|`. Column padding is optional.
- The table ends at the first blank line. A non-blank line directly after the table is a malformed row and an error.
- A key the schema does not define for the item's type is an error, even when another type defines it. A value outside a field's allowed set is an error. There is no "allow unknown keys" setting: a new field is declared in the schema.
- Keys and enum values are written as labels derived from schema names: `_` becomes a space and the first letter is capitalized (`derives_from` → `Derives from`, `almost-certain` → `Almost-certain`, type `requirement` → `Requirement`). Reading matches labels case-insensitively; writing uses the derived label.
