# Authoring verification

---

<a id="01M7F74HWB3KEQCABC1WSAJ34H"></a>

### Create items at each position

| Type | Verification |
| --- | --- |
| ID | VER-CREATE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CREATE-ITEM](authoring.mara.md#01M7BF6GKV9FQZBDWDZQJVE164) |

Rust integration tests run `item create` with the real CLI on a disposable project. A request that supplies a title, status, required fields and body, and an ID that no item has or none, creates an item that has no diagnostic and that `get` of its ID returns.

| Case | Expectation |
| --- | --- |
| No position | The item ends the document |
| Line 1, a line in narrative, and a separator line between two items | The item starts at that line, after the separator that the writer puts there when the line before needs one ([DES-ITEM-WRITER](authoring.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)); the content around it is unchanged and no neighbour merges with it |
| A line inside an item | Refused |
| A new path that the include patterns match | The document is created and does not start with `---` |
| A new path outside the include patterns, ignored by a `.gitignore`, or below another project's root | Refused; no file or directory is created |
| An ID given | The item has that ID |
| No ID, title "Phishing-resistant auth", type prefix `REQ-` | The item has the ID `REQ-PHISHING-RESISTANT-AUTH` |
| Two creates | Each anchor holds a new canonical ULID that no other item has |
| A MID supplied as an option | Rejected with `invalid-argument` and exit status 2 |

---

<a id="01M7F74HX5JT4JVYSSWJ2990YZ"></a>

### Update one part of an item at a time

| Type | Verification |
| --- | --- |
| ID | VER-UPDATE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-UPDATE-ITEM](authoring.mara.md#01M7BF6GM3N7AHGMTQ9JMT69ZP) |

Rust integration tests run `item update` with the real CLI on a disposable project. The updated item has relations, a single-valued field, a repeatable field and a body; another item shares its file. `Status` counts as a field value.

| Case | Expectation |
| --- | --- |
| A new title | Only the heading text changes |
| A new `Status`, a new value of the single-valued field, and new values of the repeatable field | Only those rows change, one row per value |
| A new body | Only the body changes |
| `--clear-field` of the repeatable field | Only its rows go |
| Title, a field and the body in one request | Those three change |
| After each of these | The anchor, the `Type` and `ID` rows, the relation rows and the other item are byte-identical |
| A request that names a new type, ID or MID, or a relation | Rejected or refused; files byte-identical |

---

<a id="01M7F74HXVC6WYBTNRTHB7AP1W"></a>

### Delete only an item nothing else targets

| Type | Verification |
| --- | --- |
| ID | VER-DELETE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-DELETE-ITEM](authoring.mara.md#01M7BF6GMX0QVD78B1ZTMY7PKF) |

Rust integration tests run `item delete` with the real CLI on a disposable project.

| Case | Expectation |
| --- | --- |
| An item with outgoing relations and a mention of itself, which nothing else targets | Removed; `get` of its ID and of its MID fails with `unknown-item`; no other item or narrative changes and the project gains no diagnostic |
| An item targeted by a relation in another item (one through a stale path), a mention in another item's body and a mention in narrative | Refused, listing each of these locations with path and line; files byte-identical |
| An item whose ID a code marker names, from 0.4.0-alpha.5 (mara-yhxq5x) | Refused, listing the marker's location |

---

<a id="01M7F74HYAKH2G83A806Y7A4HC"></a>

### Add and remove relation rows

| Type | Verification |
| --- | --- |
| ID | VER-EDIT-RELATIONS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-EDIT-RELATIONS](authoring.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |

Rust integration tests run `relation add` and `relation remove` with the real CLI on a disposable project whose schema allows some relations between its types and not others.

| Case | Expectation |
| --- | --- |
| `relation add` of an allowed relation to an item in the same file, and to one in another directory | One new row on the source item: the relation's label, then `[<ID>](#<MID>)` or `[<ID>](<relative path>#<MID>)`; `relation list` of the target shows it under the inverse name; the project gains no diagnostic |
| `relation add` of a relation the schema does not allow from the source's type to the target's | Refused; files byte-identical |
| `relation add` and `relation remove` given an inverse name | Refused, naming the forward request; files byte-identical |
| `relation remove` of an existing relation | That row is deleted and nothing else changes |

---

<a id="01M7F74HYRP3X859VN0623J9AV"></a>

### Move an item with its links

| Type | Verification |
| --- | --- |
| ID | VER-MOVE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-MOVE-ITEM](authoring.mara.md#01M7BF6GMB197M9NE49NZN0A29) |

Rust integration tests run `item move` with the real CLI on a disposable project. The moved item has a relation and a mention to an item in its own file and to one in another directory. Items in its file, in the destination file and in a third file target it with relations and mentions.

| Case | Expectation |
| --- | --- |
| Move to a document in another directory | The anchor, heading, rows and body are unchanged, except that the paths of the item's own links resolve from the new file; every inbound relation link and mention carries the new path, `#<MID>` within the destination; the project gains no diagnostic |
| Move to another position in the same document | The item is at that position, byte-identical; the document's other regions keep their contents |
| `get` and `relation list` of the MID after each move | The item at its new location, with every relation and mention |

---

<a id="01M7F74HZCX3RZJVCNDJEKVW18"></a>

### Rename an ID wherever it is a handle

| Type | Verification |
| --- | --- |
| ID | VER-RENAME-ID |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-RENAME-ID](authoring.mara.md#01M7BF6GMNEZ9PYCWYKQZGY9YN) |

Rust integration tests run `item rename` with the real CLI on a disposable project. Other items target the renamed item with relations, with mentions whose text is its ID and with mentions whose text is other words. Narrative mentions it with its ID as the text, and a paragraph holds its ID as plain text.

| Case | Expectation |
| --- | --- |
| The rename | The anchor keeps the MID; the `ID` row, the text of every inbound relation link and of every mention whose text was the old ID carry the new ID; other mention text and the plain text are unchanged; the project gains no diagnostic |
| `get`, `item validate` and `relation add` given the old ID afterwards | Fail with `unknown-item` |
| The rename stopped before its `ID` row, and after it, in the order of [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) | As [VER-INTERRUPTED-EDITS](verification-writes.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) requires |
| A code marker that names the old ID, from 0.4.0-alpha.5 (mara-yhxq5x) | Names the new ID |
