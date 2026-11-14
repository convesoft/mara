# Read requests and results

## Design

---

<a id="01M7DMT8FTCKV3J0TP5H3S3AFA"></a>

### Search and list requests

| Type | Design |
| --- | --- |
| ID | DES-SEARCH-REQUESTS |
| Status | Accepted |
| Satisfies | [REQ-SEARCH-ITEMS-AND-NARRATIVE](retrieval.mara.md#01M7BF6HA7YSRRQ2BWS54AEGSG) |
| Satisfies | [REQ-SEARCH-RANKING](retrieval.mara.md#01M7BF6HAHQJNEGTAV5A2AY9RX) |
| Satisfies | [REQ-LIST-ITEMS](retrieval.mara.md#01M7BF6HBTZ6YD1XBDXRD4ZCC4) |

| Filter | Value | Matches |
| --- | --- | --- |
| `--type` | A type name | Items of that type |
| `--field` | `<key>=<value>` | Items with a row of that key and value |
| `--relation` | A forward or inverse relation name | Items that an edge of that relation leaves, for a forward name, or targets, for an inverse name |
| `--path` | A document or a directory, from the project root, read as `project validate` reads it ([DES-DIAGNOSTICS](validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)) | Nodes in that document, or in a document below that directory |
| `--id` | An `ID` value or a MID | The items that have it |

- Every filter may repeat. Values of one filter are alternatives; different filters, and field filters of different keys, all apply.
- Keys, type and relation names, and `Status` and enum values match as labels do ([DES-METADATA-TABLE](format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)); any other value matches as written.
- A type, field, relation or ID filter matches no narrative unit.
- The query is one argument, read as words like the documents ([DES-SNAPSHOT](engine.mara.md#01M7BF6GYM8NPPGY9RZRF38EGE)). A query without words matches every node.
- Within each group of [REQ-SEARCH-RANKING](retrieval.mara.md#01M7BF6HAHQJNEGTAV5A2AY9RX), a node with more query words matched in its ID, title or a heading comes first. In the group of exact matches only exact ones count.
- The excerpt is the line of the node's first occurrence of a query word, else of a word within the allowed edits of one, as the token index records the words and where they start ([DES-SNAPSHOT](engine.mara.md#01M7BF6GYM8NPPGY9RZRF38EGE)); without query words, the node's first line of text other than its anchor. It shows at most 240 characters of that line, from up to 60 before the word. A word in a code span that runs over several lines has the position of the code span's start, so its excerpt is the line where the span starts.

---

<a id="01M7DMT8GAJCAFTW0Y2939JFF1"></a>

### Read results

| Type | Design |
| --- | --- |
| ID | DES-READ-RESULTS |
| Status | Accepted |
| Satisfies | [REQ-CLI-MCP-PARITY](hosts.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |
| Satisfies | [REQ-BOUNDED-PAGES](retrieval.mara.md#01M7BF6HC3AE196XHCMXGKJXFS) |
| Satisfies | [REQ-UNAVAILABLE-NOT-PARTIAL](engine.mara.md#01M7BF6GXSJVVASJ4C8ZGHM909) |

| Operation | Takes | Records |
| --- | --- | --- |
| `search` | A query, filters, `--limit`, `--cursor` | `results`: a node with `excerpt` (`line`, `text`) |
| `get` | A node, `--cursor` | None: `node`, then this page's `rows` (`key`, `value`, `line`, `partial`) and `content` |
| `relation list` | A node, `--relation`, `--limit`, `--cursor` | `connections`, after `node` |
| `item list` | Filters, `--limit`, `--cursor` | `items`: item nodes |
| `schema list` | `--limit`, `--cursor` | `declarations`: `kind`, `name`, `id_prefix` or `inverse`, `description` |
| `schema get` | A type name, or a relation's forward or inverse name; `--limit`, `--cursor` | `declarations`: each declaration of that name as loaded ([DES-SCHEMA-MODEL](project.mara.md#01M7BF6HA0KG3R3AATWM1P2X1G)), a type with the `Status` values |

- A result is a JSON object of `format_version` (1), its operation's members, `diagnostics` ([DES-DIAGNOSTICS](validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)), `diagnostics_total` and `next_cursor`, which is `null` on the last page. The human form prints the records, then those diagnostics and, when some are left out, how many with a pointer to `mara project validate`, then the next cursor.
- A node is `{kind: "item", path, line, id, mid, type, title}`, `{kind: "narrative", path, line, heading}` or `{kind: "document", path}`. `get` adds an item's `revision`: the lowercase hexadecimal BLAKE3 hash of its region.
- A connection is `{kind, relation, direction, neighbour, source}`: `kind` is `relation` or `mention`, `relation` the name shown or `null`, `direction` `outgoing` or `incoming`, `neighbour` a node or `{kind: "unresolved", mid}`, `source` the `path` and `line` of the link.
- `relation list` lists an item's outgoing relations in row order and its mentions in source order, then the relations and mentions that target it in path and source order; a narrative unit's mentions; and every relation and mention written in a whole document, in source order. A link to a MID that several items have connects to each of them.
- A node is named by an ID, a MID, `<path>:<line>` or `<path>`; `get` and `relation list` take every one. An ID or MID that several items have names none: the read fails with their locations. Failure codes are those of [DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS); `record-too-large` carries a diagnostic at the record's location.
- A read of documents carries the inventory's diagnostics ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)) on every page: the first of them in path and line order, as many as fit in 8,192 bytes of JSON, and `diagnostics_total`, how many there are. Its result is complete only when the total is 0; `project validate` pages every one. So every page of every read, with records or without, stays within the page's bytes.
