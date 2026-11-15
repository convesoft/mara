# Authoring

## Requirements

---

<a id="01M7BF6GKV9FQZBDWDZQJVE164"></a>

### Create an item

| Type | Requirement |
| --- | --- |
| ID | REQ-CREATE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Creating an item writes a complete item with a newly generated MID into a named document, at its end or at a given line outside every existing item. It creates the document when none exists and the path would be an input ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). The caller supplies an ID or accepts one proposed from the type prefix and a title slug, and never supplies a MID.

---

<a id="01M7BF6GM3N7AHGMTQ9JMT69ZP"></a>

### Update an item

| Type | Requirement |
| --- | --- |
| ID | REQ-UPDATE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Updating an item replaces only the title, field values or body named in the request. Its MID, type and relations stay as they are.

---

<a id="01M7BF6GMB197M9NE49NZN0A29"></a>

### Move an item

| Type | Requirement |
| --- | --- |
| ID | REQ-MOVE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Moving an item to another document or position keeps its MID and content. It rewrites the path of every inbound link and of the item's own outbound links.

---

<a id="01M7BF6GMNEZ9PYCWYKQZGY9YN"></a>

### Rename an ID

| Type | Requirement |
| --- | --- |
| ID | REQ-RENAME-ID |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Renaming an ID keeps the MID and rewrites the text of every inbound relation link, every mention whose text is exactly the old ID, and every code marker that names the old ID ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)). It leaves no alias: the old ID no longer resolves.

---

<a id="01M7BF6GMX0QVD78B1ZTMY7PKF"></a>

### Delete an item

| Type | Requirement |
| --- | --- |
| ID | REQ-DELETE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Deleting an item is refused while a relation or mention in another item or in narrative targets its MID, or a code marker names its ID. The refusal lists each such location.

---

<a id="01M7BF6GN8DJ66NPBMQK83H9PW"></a>

### Add and remove relations

| Type | Requirement |
| --- | --- |
| ID | REQ-EDIT-RELATIONS |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Adding a relation writes one canonical link row on the source item and is refused when the schema does not allow that relation between the two types. Removing a relation deletes that row. Both take only the forward relation name: a request with an inverse name is refused and names the forward request.

---

<a id="01M7BF6GNPRRHNP2TRX7MKZRRH"></a>

### One fix operation repairs source

| Type | Requirement |
| --- | --- |
| ID | REQ-ONE-FIX-OPERATION |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

A single explicit fix operation adds the anchor an item lacks and rewrites bare-ID relation values, stale link paths and link text, and malformed item formatting. It never deletes an item and never supplies a missing heading, type or ID. No other operation repairs source.

---

## Design

---

<a id="01M7BF6GP2Z6R1VCQMQ6ZZB218"></a>

### One writer for item syntax

| Type | Design |
| --- | --- |
| ID | DES-ITEM-WRITER |
| Status | Accepted |
| Satisfies | [REQ-CREATE-ITEM](#01M7BF6GKV9FQZBDWDZQJVE164) |
| Satisfies | [REQ-ONE-FIX-OPERATION](#01M7BF6GNPRRHNP2TRX7MKZRRH) |

- A single writer produces every anchor, metadata table and relation link. No operation formats item syntax itself.
- It generates the MID together with the anchor, and writes labels and links in their canonical form ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD), [DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)).
- Updates and moves keep the existing anchor.
- A MID is the ULID of the host's time in milliseconds and 80 bits from the operating system's random source.
- New lines take the line ending of the document's first line, LF without one; a row added to a table takes that of the table's last row.
- The built-in keys are written `Type`, `ID` and `Status`, the `Type` row as the header row.
- A title or cell value that is empty, holds a line break, or starts or ends with a space or tab is refused, since it would not read back. A cell writes `|` as `\|`; a heading escapes a closing run of `#`.
- Link text escapes ASCII punctuation other than `-` with a backslash, so a valid ID is written as it is. A link path percent-encodes controls, the space and `"%&#?()<>\|:`, which validation decodes; other characters stay as written. An encoded `:` keeps the part before it from reading as a URL scheme.
- An item is inserted at a line start as a region of its own: a `---` line with a blank line on each side before it, unless an existing one is there or only front matter precedes it, and after it, unless it ends the file. A closing front matter line without a line ending gets one. Removing an item removes one separator with it, the one after it when the item comes first, so no file starts with `---` ([DES-REGIONS](format.mara.md#01M7BF6GZ5V5HWV00MDZN68Z06)). Text before a merged anchor on its line stays, and a line right after the item keeps a blank line before it.
- Edits of an existing item splice at the parser's offsets of its rows, values and table. Removing the header row moves the next row's text into its place, with a delimiter row of as many columns as that row has cells.
- Every edit of a document is read back before it is written ([ADR-READ-BACK-EVERY-EDIT](#01M7FH6HEWNYQ66F5KE912BQSM)): one parse of each distinct new content of the edit, before the first rename, against the parse the edit was computed from. Only the bytes that the edit's splices replace or insert may read differently; an item, narrative unit, table or row that a splice merely touches gets no exemption for its other bytes. It refuses the edit with `unreadable-edit`, naming what would read differently and why, unless:
  - the front matter is the same;
  - every item whose anchor the edit leaves alone is read where the anchor moved, with its title unless the edit changes its heading, and ending where its end moved unless the edit writes there;
  - every metadata row, every link, image and footnote reference, and every top-level block whose own bytes the edit leaves alone reads as before apart from offsets, in the same item or narrative: a row's key, value and link, a reference's destination, title and footnote definition, and a block's start, and its extent unless the edit writes at one of its ends;
  - nothing new reads as an item, a row, a reference or a block outside the bytes the edit writes;
  - each removal takes one item and only blank lines, spaces and separators around it, and each item the edit changes has the rows its row changes give it;
  - an inserted item goes between blocks and link reference definitions, starts where it was put, ends within its text, and each of its references that a definition decides uses one inside it; the bytes after its last block, such as definitions and trailing spaces, stay;
  - no parse problem appears outside an inserted item.
- Link reference and footnote definitions are document-wide, so an edit in one place can change how another reads: removing, changing, adding or shadowing one that other text uses is refused, and so is an item that a code block, HTML block or comment would take in. An edit that means to change what other text reads as writes that text too: to retarget links that a definition decides, it rewrites the definition and the links in one splice, or each link.

---

## Decisions

---

<a id="01M7FH6HEWNYQ66F5KE912BQSM"></a>

### Read every edit back before writing it

| Type | Decision |
| --- | --- |
| ID | ADR-READ-BACK-EVERY-EDIT |
| Status | Accepted |
| Justifies | [DES-ITEM-WRITER](#01M7BF6GP2Z6R1VCQMQ6ZZB218) |

- Chosen by the maintainer on 2026-11-15: every edit is verified by reading it back, so no edit can silently lose or swallow an item, whatever the document looks like. It costs one parse of each changed document per write, before the renames and nothing after them, which [VER-WORK-COUNTERS](verification.mara.md#01M7BF6HGG6ET88HS641Y5676G) counts. A host that keeps state can reuse that parse.
- Rejected: teaching each editing function about open code blocks, HTML blocks and comments. Any case that nobody thought of would be silent data loss.
- Chosen by the maintainer on 2026-11-15, after two review rounds found untouched text that an accepted edit made read differently: the read-back exempts only the exact bytes that the edit's splices replace or insert and compares everything else in the document, with no exemption for an item or narrative that a splice merely touches.
- Rejected: patching each case a review finds, which left the same defect open for two rounds.
