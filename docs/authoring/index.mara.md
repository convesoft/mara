# Authoring

| Page | Holds |
| --- | --- |
| [Write safety](writes.mara.md) | What a write preserves, write order |
| [Authoring verification](verification.mara.md) | Checks of create, update, delete, relation edits, move, rename, preserved bytes, refused, interrupted and concurrent writes, and fix |

## Requirements

---

<a id="01M7BF6GKV9FQZBDWDZQJVE164"></a>

### Create an item

| Type | Requirement |
| --- | --- |
| ID | REQ-CREATE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Creating an item writes a complete item with a newly generated MID into a named document, at its end or at a given line outside every existing item. It creates the document when none exists and the path would be an input ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). The caller supplies an ID or accepts one proposed from the type prefix and a title slug, and never supplies a MID.

---

<a id="01M7BF6GM3N7AHGMTQ9JMT69ZP"></a>

### Update an item

| Type | Requirement |
| --- | --- |
| ID | REQ-UPDATE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Updating an item replaces only the title, field values or body named in the request. Its MID, type and relations stay as they are.

---

<a id="01M7BF6GMB197M9NE49NZN0A29"></a>

### Move an item

| Type | Requirement |
| --- | --- |
| ID | REQ-MOVE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Moving an item to another document or position keeps its MID and content. It rewrites the path of every inbound link and of the item's own outbound links.

---

<a id="01M7BF6GMNEZ9PYCWYKQZGY9YN"></a>

### Rename an ID

| Type | Requirement |
| --- | --- |
| ID | REQ-RENAME-ID |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Renaming an ID keeps the MID and rewrites the text of every inbound relation link, every mention whose text is exactly the old ID, and every code marker that names the old ID ([DES-CODE-MARKERS](../code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)). It leaves no alias: the old ID no longer resolves.

---

<a id="01M7BF6GMX0QVD78B1ZTMY7PKF"></a>

### Delete an item

| Type | Requirement |
| --- | --- |
| ID | REQ-DELETE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Deleting an item is refused while a relation or mention in another item or in narrative targets its MID, or a code marker names its ID. The refusal lists each such location.

---

<a id="01M7BF6GN8DJ66NPBMQK83H9PW"></a>

### Add and remove relations

| Type | Requirement |
| --- | --- |
| ID | REQ-EDIT-RELATIONS |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Adding a relation writes one canonical link row on the source item and is refused when the schema does not allow that relation between the two types. Removing a relation deletes that row. Both take only the forward relation name: a request with an inverse name is refused and names the forward request.

---

<a id="01M7BF6GNPRRHNP2TRX7MKZRRH"></a>

### One fix operation repairs source

| Type | Requirement |
| --- | --- |
| ID | REQ-ONE-FIX-OPERATION |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

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
- It generates the MID together with the anchor, and writes labels and links in their canonical form ([DES-METADATA-TABLE](../format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD), [DES-RELATION-LINKS](../identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)).
- Updates and moves keep the existing anchor.
- A MID is the ULID of the host's time in milliseconds and 80 bits from the operating system's random source.
- New lines take the line ending of the document's first line, LF without one; a row added to a table takes that of the table's last row.
- The built-in keys are written `Type`, `ID` and `Status`, the `Type` row as the header row.
- A title or cell value that is empty, holds a line break, or starts or ends with a space or tab is refused, since it would not read back. A cell writes `|` as `\|`; a heading escapes a closing run of `#`.
- Link text escapes ASCII punctuation other than `-` with a backslash, so a valid ID is written as it is. A link path percent-encodes controls, the space and `"%&#?()<>\|:`, which validation decodes; other characters stay as written. An encoded `:` keeps the part before it from reading as a URL scheme.
- An item is inserted at a line start as a region of its own: a `---` line with a blank line on each side before it, unless an existing one is there or only front matter precedes it, and after it, unless it ends the file. A closing front matter line without a line ending gets one. Removing an item removes one separator with it, the one after it when the item comes first, so no file starts with `---` ([DES-REGIONS](../format.mara.md#01M7BF6GZ5V5HWV00MDZN68Z06)). Text before a merged anchor on its line stays, and a line right after the item keeps a blank line before it.
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

<a id="01M7FWNBFDHGXN2TH1BK08GKCK"></a>

### Move and rename edits

| Type | Design |
| --- | --- |
| ID | DES-MOVE-AND-RENAME |
| Status | Accepted |
| Satisfies | [REQ-MOVE-ITEM](#01M7BF6GMB197M9NE49NZN0A29) |
| Satisfies | [REQ-RENAME-ID](#01M7BF6GMNEZ9PYCWYKQZGY9YN) |

- `item move <id-or-mid> <path> [--line <n>]`, as in 0.3.0: the path is relative to the project root; the item goes before line `n` of the destination as it is now, a line outside every item, or at its end. A line inside an item or past the end is refused with `invalid-position`.
- A destination that is no document yet is created when it would be an input ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)); otherwise the move is refused with `not-an-input`, or `symbolic-link` for a link on its path.
- The moved item keeps its bytes, line endings included. Its own relation links and mentions name the same files from the destination, those to itself the destination; every relation link and mention to it elsewhere names the destination. Only path bytes change, and a link whose path already resolves to the right file keeps them. A link that a definition decides gets its path through the definition, written with every link and image that uses it ([DES-ITEM-WRITER](#01M7BF6GP2Z6R1VCQMQ6ZZB218)).
- Other relative links of the moved item, to files that are no documents or to images, keep their bytes, so a move to another directory breaks them.
- A link, image or footnote reference of the moved item that uses a definition outside the item, after its last block or at the end of the page, refuses the move with `unreadable-edit` at that reference: the definition would stay behind.
- A move to where the item is, a line among the blank lines and separators around it, writes no item.
- An ID or MID that several items have is refused with `ambiguous-item`, except for the two copies that an interrupted move leaves ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)).
- `item rename <id-or-mid> <new-id>`: a new ID without the form of [DES-ITEM-IDS](../identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS) is rejected with `invalid-argument`; one without the prefix of the item's type, or that another item has, is refused with the code of the diagnostic it would cause, `wrong-id-prefix` or `duplicate-id`. An item without an `ID` row is refused with `missing-row`.
- A rename rewrites the `ID` rows that hold the old ID, the text of each mention that reads exactly the old ID and the text of each relation link to the item that does not read the new ID. Inside the brackets, the old text written once as it reads is replaced, so emphasis stays; otherwise the whole text is. A collapsed or shortcut reference becomes the full reference `[<new ID>][<label>]`, which uses the same definition.
- Both results hold the item as `get` summarizes it, where it is afterwards, `files` in write order and the inventory's `diagnostics`; a move adds `from` with its former path and line, a rename `old_id`.

---

<a id="01M7FWQ5YB6X95VCRVSP1FGQ46"></a>

### Single-file write requests and results

| Type | Design |
| --- | --- |
| ID | DES-AUTHORING-REQUESTS |
| Status | Accepted |
| Satisfies | [REQ-CREATE-ITEM](#01M7BF6GKV9FQZBDWDZQJVE164) |
| Satisfies | [REQ-UPDATE-ITEM](#01M7BF6GM3N7AHGMTQ9JMT69ZP) |
| Satisfies | [REQ-DELETE-ITEM](#01M7BF6GMX0QVD78B1ZTMY7PKF) |
| Satisfies | [REQ-EDIT-RELATIONS](#01M7BF6GN8DJ66NPBMQK83H9PW) |
| Satisfies | [REQ-CLI-MCP-PARITY](../hosts/index.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |

| Operation | Takes |
| --- | --- |
| `item create` | A type, a document path, `--title`, `--body`, `--id`, `--field <key>=<value>` (repeatable), `--line <n>` |
| `item update` | An item, and at least one of `--title`, `--field <key>=<value>` (repeatable), `--clear-field <name>` (repeatable), `--body` |
| `item delete` | An item |
| `relation add`, `relation remove` | A source item, a relation name, a target item |

- An item is named by its ID, its MID or `<path>:<line>`, as [DES-READ-RESULTS](../retrieval/index.mara.md#01M7DMT8GAJCAFTW0Y2939JFF1) names a node. Type, relation, key and enum or `Status` value names match as labels do ([DES-METADATA-TABLE](../format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)); values are written as labels.
- `Status` is a `--field` key. `--field type=…` and `--field id=…` are rejected arguments: the type is fixed at creation, and `item rename` changes the ID. `item create` needs `--title` and `--body`.
- Each operation runs the write procedure ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) and writes one document. The engine computes the edit and reads it back; the host checks a new document's path, generates the MID and writes.
- A result is a JSON object of `format_version` (1), `item`, the node of the item created, updated or deleted, or of the relation's source, as written (as it was, for a delete), and `files`, the paths written. A relation result adds `relation` (the forward name), `target` (a node) and `line`, that of the row written or removed. The human form is one line.
- An update that changes no byte writes nothing, and its `files` is empty.

Create:

- An ID given must have the form of [DES-ITEM-IDS](../identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS), the type's prefix, and belong to no item. Without one, the proposal is the prefix, then the title's ASCII letters and digits in uppercase, each run of other characters one `-`: "Phishing-resistant auth" gives `REQ-PHISHING-RESISTANT-AUTH`. A proposal that is no valid ID or that an item has is refused, asking for `--id`.
- The rows are `Type`, `ID`, `Status`, then the fields in the order given. `Status` and every required field must be given, and the body must hold text.
- The heading level is that of the nearest item before the position, else after it, else 2.
- `--line` takes 1 to the line after the last; without it the item ends the document. A line inside an item or further on is refused with `invalid-position`. The item goes at the start of that line, after a separator when the line before needs one ([DES-ITEM-WRITER](#01M7BF6GP2Z6R1VCQMQ6ZZB218)).
- A path that no document has is created when it would be an input, as for a move ([DES-MOVE-AND-RENAME](#01M7FWNBFDHGXN2TH1BK08GKCK)); otherwise the create is refused with `not-an-input`, or `symbolic-link` for a link on its path.

Update:

- The values of one key replace all its rows in order: a row's value is set in place, surplus rows go from the last, and extra values become rows after the last row that stays. A key that is a relation, undeclared for the type, or given an undeclared value is refused.
- `--clear-field` removes every row of an optional field, as 0.3.0 did; a field the item lacks changes nothing. `Type`, `ID` and `Status`, a name that `--field` also sets, and an empty `--field` value are rejected arguments; a required field, a relation and a key the type does not declare are refused with `invalid-item`.
- The title replaces the heading's text and keeps its level and markers; the body replaces the blocks after the table.

Create and update:

- A value outside a field's allowed set, several values of a single-valued key, and a value given twice are refused, as validation would report them ([DES-SCHEMA-CHECKS](../validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)). Links in a title or body are not checked; validation reports them.
- A title or body after which the item would not read back as one item, or would read a parse problem it lacked, is refused with `unreadable-edit`: a thematic break, an anchor, or a heading followed by an item's table in a body, or a setext underline. A missing separator is named before the problems it brings.
- A body that ends inside an open fenced code block is written when nothing follows the item; any later item after it is then refused until the fence is closed.

Delete:

- Each incoming relation or mention from another item or from narrative is a diagnostic of the refusal, at its link. The item's links to itself do not count. The document stays, also when it is left empty.

Relations:

- `relation add` writes `| <Label> | [<ID>](<path>#<MID>) |` after the source table's last row ([DES-RELATION-LINKS](../identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)). It is refused when the schema does not lead the relation from the source's type to the target's, when the source already has that row, and when the target has no ID. A cycle of an acyclic relation is left to validation.
- `relation remove` deletes every row of that relation on the source whose link targets the target's MID, whatever its path or text. Both are refused with `invalid-item` when the source's type is no source of the relation, whose rows of that name are fields or undeclared keys.
- An inverse name is refused with the forward request, `mara relation <verb> <target> <forward name> <source>`.

Failure codes, besides those of [DES-OPERATIONS](../hosts/index.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS):

| Code | Failure |
| --- | --- |
| `invalid-item` | The request would write an item or row that the schema or the identity rules do not allow, or a relation row that is there already |
| `inverse-relation` | A relation given by its inverse name; the message names the forward request |
| `targeted-item` | A delete of an item that another item or narrative targets; `diagnostics` holds each link |
| `missing-relation` | A `relation remove` of a row the source does not have |

---

<a id="01M7FWM48HHP0HHM6S86P6R499"></a>

### Fix repairs

| Type | Design |
| --- | --- |
| ID | DES-FIX-REPAIRS |
| Status | Accepted |
| Satisfies | [REQ-ONE-FIX-OPERATION](#01M7BF6GNPRRHNP2TRX7MKZRRH) |

`project fix` takes no argument and refuses a project whose configuration or schema has an error, as every operation but validation does ([DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)). It repairs these problems and no other:

| Problem | Repair |
| --- | --- |
| `incomplete-item` of a region that starts with a heading followed by a table with a `Type` row | An anchor with a new MID that no item has, and a blank line, before the heading, in a splice that holds the whole region |
| `missing-separator` | A `---` line before the anchor, with a blank line on each side; before a heading, in the splice of that region's anchor; before inline HTML in the heading's place, alone |
| `merged-anchor` of an anchor that lacks only the blank line after it | That blank line |
| `setext-heading` of one line of text in an item's heading place | An ATX heading of the same level and text |
| `bare-id-value` that names one item | The relation link to it ([DES-ITEM-WRITER](#01M7BF6GP2Z6R1VCQMQ6ZZB218)) |
| `stale-link-path` or `stale-link-text` of a relation | The relation link, as the writer writes it: the target's ID, or the link's text when the target has none |
| `stale-link-path` of a mention | Only the path; for a reference link, the path in its definition, in one splice with every link that uses it |

- A link whose destination is written with backslash escapes or character references stays as it is, still reported.
- An item that gets its anchor has its heading, relation values and mentions repaired in the same run, and a bare ID may name it, so one run leaves none of these problems.
- It writes the changed documents in path order. Each document's repairs depend on no other document, so a rerun completes an interrupted fix. A document whose repairs would not read back refuses the whole fix with `unreadable-edit`.
- The result is `format_version` (1), `repairs` and `repairs_total`. A repair is the problem's `code`, `path`, `line` before the fix and `message`; `repairs` holds the first in path and line order that fit in 8,192 bytes of JSON. The human form prints each as `<path>:<line>: fixed[<code>]: <message>`, then the counts. Exit status 0.

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

- Chosen by the maintainer on 2026-11-15: every edit is verified by reading it back, so no edit can silently lose or swallow an item, whatever the document looks like. It costs one parse of each changed document per write, before the renames and nothing after them, which [VER-WORK-COUNTERS](../verification.mara.md#01M7BF6HGG6ET88HS641Y5676G) counts. A host that keeps state can reuse that parse.
- Rejected: teaching each editing function about open code blocks, HTML blocks and comments. Any case that nobody thought of would be silent data loss.
- Chosen by the maintainer on 2026-11-15, after two review rounds found untouched text that an accepted edit made read differently: the read-back exempts only the exact bytes that the edit's splices replace or insert and compares everything else in the document, with no exemption for an item or narrative that a splice merely touches.
- Rejected: patching each case a review finds, which left the same defect open for two rounds.
