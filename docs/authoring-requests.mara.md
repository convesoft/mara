# Write requests and results

## Design

---

<a id="01M7FWQ5YB6X95VCRVSP1FGQ46"></a>

### Single-file write requests and results

| Type | Design |
| --- | --- |
| ID | DES-AUTHORING-REQUESTS |
| Status | Accepted |
| Satisfies | [REQ-CREATE-ITEM](authoring.mara.md#01M7BF6GKV9FQZBDWDZQJVE164) |
| Satisfies | [REQ-UPDATE-ITEM](authoring.mara.md#01M7BF6GM3N7AHGMTQ9JMT69ZP) |
| Satisfies | [REQ-DELETE-ITEM](authoring.mara.md#01M7BF6GMX0QVD78B1ZTMY7PKF) |
| Satisfies | [REQ-EDIT-RELATIONS](authoring.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |
| Satisfies | [REQ-CLI-MCP-PARITY](hosts.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |

| Operation | Takes |
| --- | --- |
| `item create` | A type, a document path, `--title`, `--body`, `--id`, `--field <key>=<value>` (repeatable), `--line <n>` |
| `item update` | An item, and at least one of `--title`, `--field <key>=<value>` (repeatable), `--clear-field <name>` (repeatable), `--body` |
| `item delete` | An item |
| `relation add`, `relation remove` | A source item, a relation name, a target item |

- An item is named by its ID, its MID or `<path>:<line>`, as [DES-READ-RESULTS](retrieval-requests.mara.md#01M7DMT8GAJCAFTW0Y2939JFF1) names a node. Type, relation, key and enum or `Status` value names match as labels do ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)); values are written as labels.
- `Status` is a `--field` key. `--field type=…` and `--field id=…` are rejected arguments: the type is fixed at creation, and `item rename` changes the ID. `item create` needs `--title` and `--body`.
- Each operation runs the write procedure ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) and writes one document. The engine computes the edit and reads it back; the host checks a new document's path, generates the MID and writes.
- A result is a JSON object of `format_version` (1), `item`, the node of the item created, updated or deleted, or of the relation's source, as written (as it was, for a delete), and `files`, the paths written. A relation result adds `relation` (the forward name), `target` (a node) and `line`, that of the row written or removed. The human form is one line.
- An update that changes no byte writes nothing, and its `files` is empty.

Create:

- An ID given must have the form of [DES-ITEM-IDS](identity.mara.md#01M7BF6H1ZBTKWTF9ZXEJ8EBGS), the type's prefix, and belong to no item. Without one, the proposal is the prefix, then the title's ASCII letters and digits in uppercase, each run of other characters one `-`: "Phishing-resistant auth" gives `REQ-PHISHING-RESISTANT-AUTH`. A proposal that is no valid ID or that an item has is refused, asking for `--id`.
- The rows are `Type`, `ID`, `Status`, then the fields in the order given. `Status` and every required field must be given, and the body must hold text.
- The heading level is that of the nearest item before the position, else after it, else 2.
- `--line` takes 1 to the line after the last; without it the item ends the document. A line inside an item or further on is refused with `invalid-position`. The item goes at the start of that line, after a separator when the line before needs one ([DES-ITEM-WRITER](authoring.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)).
- A path that no document has is created when it would be an input, as for a move ([DES-MOVE-AND-RENAME](authoring.mara.md#01M7FWNBFDHGXN2TH1BK08GKCK)); otherwise the create is refused with `not-an-input`, or `symbolic-link` for a link on its path.

Update:

- The values of one key replace all its rows in order: a row's value is set in place, surplus rows go from the last, and extra values become rows after the last row that stays. A key that is a relation, undeclared for the type, or given an undeclared value is refused.
- `--clear-field` removes every row of an optional field, as 0.3.0 did; a field the item lacks changes nothing. `Type`, `ID` and `Status`, a name that `--field` also sets, and an empty `--field` value are rejected arguments; a required field, a relation and a key the type does not declare are refused with `invalid-item`.
- The title replaces the heading's text and keeps its level and markers; the body replaces the blocks after the table.

Create and update:

- A value outside a field's allowed set, several values of a single-valued key, and a value given twice are refused, as validation would report them ([DES-SCHEMA-CHECKS](validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)). Links in a title or body are not checked; validation reports them.
- A title or body after which the item would not read back as one item, or would read a parse problem it lacked, is refused with `unreadable-edit`: a thematic break or an anchor in a body, or a setext underline.
- A body that ends inside an open fenced code block is written when nothing follows the item; any later item after it is then refused until the fence is closed.

Delete:

- Each incoming relation or mention from another item or from narrative is a diagnostic of the refusal, at its link. The item's links to itself do not count. The document stays, also when it is left empty.

Relations:

- `relation add` writes `| <Label> | [<ID>](<path>#<MID>) |` after the source table's last row ([DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V)). It is refused when the schema does not lead the relation from the source's type to the target's, when the source already has that row, and when the target has no ID. A cycle of an acyclic relation is left to validation.
- `relation remove` deletes every row of that relation on the source whose link targets the target's MID, whatever its path or text. Both are refused with `invalid-item` when the source's type is no source of the relation, whose rows of that name are fields or undeclared keys.
- An inverse name is refused with the forward request, `mara relation <verb> <target> <forward name> <source>`.

Failure codes, besides those of [DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS):

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
| Satisfies | [REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH) |

`project fix` takes no argument and refuses a project whose configuration or schema has an error, as every operation but validation does ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)). It repairs these problems and no other:

| Problem | Repair |
| --- | --- |
| `incomplete-item` of a region that starts with a heading followed by a table with a `Type` row | An anchor with a new MID that no item has, and a blank line, before the heading, in a splice that holds the whole region |
| `missing-separator` | A `---` line before the anchor, with a blank line on each side |
| `merged-anchor` of an anchor that lacks only the blank line after it | That blank line |
| `setext-heading` of one line of text in an item's heading place | An ATX heading of the same level and text |
| `bare-id-value` that names one item | The relation link to it ([DES-ITEM-WRITER](authoring.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)) |
| `stale-link-path` or `stale-link-text` of a relation | The relation link, as the writer writes it: the target's ID, or the link's text when the target has none |
| `stale-link-path` of a mention | Only the path; for a reference link, the path in its definition, in one splice with every link that uses it |

- A link whose destination is written with backslash escapes or character references stays as it is, still reported.
- An item that gets its anchor has its heading, relation values and mentions repaired in the same run, and a bare ID may name it, so one run leaves none of these problems.
- It writes the changed documents in path order. Each document's repairs depend on no other document, so a rerun completes an interrupted fix. A document whose repairs would not read back refuses the whole fix with `unreadable-edit`.
- The result is `format_version` (1), `repairs` and `repairs_total`. A repair is the problem's `code`, `path`, `line` before the fix and `message`; `repairs` holds the first in path and line order that fit in 8,192 bytes of JSON. The human form prints each as `<path>:<line>: fixed[<code>]: <message>`, then the counts. Exit status 0.
