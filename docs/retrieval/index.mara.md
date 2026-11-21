# Retrieval

| Page | Holds |
| --- | --- |
| [Retrieval verification](verification.mara.md) | Checks of search, get, relation list, item list, paging and schema inspection |

## Requirements

---

<a id="01M7BF6HA7YSRRQ2BWS54AEGSG"></a>

### Search items and narrative

| Type | Requirement |
| --- | --- |
| ID | REQ-SEARCH-ITEMS-AND-NARRATIVE |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Search returns the items and narrative whose own text contains every query word, each with its location and one excerpt. Filters by type, field value, relation, path and ID match exactly.

---

<a id="01M7BF6HAHQJNEGTAV5A2AY9RX"></a>

### Rank search results reproducibly

| Type | Requirement |
| --- | --- |
| ID | REQ-SEARCH-RANKING |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Results that match every word exactly precede results that need typo tolerance. Within each group a match in the ID, title or a heading outranks a match in the body or a field, and ties follow path and source order.

---

<a id="01M7BF6HAS8Y6FSG3BXXPTZNGP"></a>

### Tolerate small spelling errors

| Type | Requirement |
| --- | --- |
| ID | REQ-TYPO-TOLERANT-SEARCH |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Search also matches a word within one edit of a query word of 4–7 characters, or two edits of a longer one. An edit inserts, deletes or substitutes one character, or swaps two adjacent characters that no other edit touches (optimal string alignment). A query word of more than 18 characters, and ID and MID values, match only exactly.

---

<a id="01M7BF6HB1DC9AFHM890ERAJCK"></a>

### Read one node

| Type | Requirement |
| --- | --- |
| ID | REQ-READ-NODE |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`get` returns one item by ID or MID, or a document or narrative unit returned by search: its content, its metadata rows in source order, its location and, for an item, its revision ([DES-FACT-BRIDGE](../rules/index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)). It returns no neighbours.

---

<a id="01M7BF6HBCR1H0P6S8KWBV3DAS"></a>

### Follow direct connections

| Type | Requirement |
| --- | --- |
| ID | REQ-DIRECT-NEIGHBOURS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`relation list` takes one node and returns the connections one hop from it: outgoing relations, incoming relations under their inverse name, mentions and code links, each with its source location. It narrows by relation name and never expands a second hop.

---

<a id="01M7BF6HBTZ6YD1XBDXRD4ZCC4"></a>

### List items

| Type | Requirement |
| --- | --- |
| ID | REQ-LIST-ITEMS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`item list` returns compact item summaries in path and source order, narrowed by the same filters as search. A summary holds the item's ID, MID, type, title, path and line.

---

<a id="01M7BF6HC3AE196XHCMXGKJXFS"></a>

### Every read is paged

| Type | Requirement |
| --- | --- |
| ID | REQ-BOUNDED-PAGES |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

A read returns at most a bounded number of records and bytes, with a cursor while more remain. Following the cursors yields each record exactly once. A cursor from a snapshot that has since changed is rejected as stale.

---

<a id="01M7CMW719S9H8FQKRFWWQ6NQK"></a>

### Page limits

| Type | Requirement |
| --- | --- |
| ID | REQ-PAGE-LIMITS |
| Status | Draft |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](../product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

A page holds 20 records unless the request asks for another number from 1 to 100, and at most 65,536 bytes; a request for another number is refused before anything runs. `get` takes no number of records: its pages are bounded by bytes alone. The numbers are those of 0.3.0 and provisional.

---

## Design

---

<a id="01M7CMW710P9V73WXFQ15PBYBS"></a>

### Narrative units

| Type | Design |
| --- | --- |
| ID | DES-NARRATIVE-UNITS |
| Status | Accepted |
| Satisfies | [REQ-SEARCH-ITEMS-AND-NARRATIVE](#01M7BF6HA7YSRRQ2BWS54AEGSG) |
| Satisfies | [REQ-READ-NODE](#01M7BF6HB1DC9AFHM890ERAJCK) |

- A narrative unit is a heading and the blocks up to the next heading in the same region. Blocks before a region's first heading form one unit.
- Every region that is no item is narrative, that of an incomplete item included. Text before an item's merged anchor is a unit of its own; the text after the anchor in its paragraph starts the item's body, so search indexes it as body text and `get` of the item returns its content from that text on, the heading and the table as written included. The item's span and its validation do not change.
- Search returns a unit as `<path>:<line>`, the line where it starts, the path being from the project root.
- `get <path>:<line>` reads the unit or item that contains that line, the one that starts first when the line holds a merged anchor. `get <path>` reads the whole document. A line that no item or unit contains, such as a `---` line, names nothing.

---

<a id="01M7BF6HCCWQH6P13ZR2YHAPS0"></a>

### Paging

| Type | Design |
| --- | --- |
| ID | DES-PAGING |
| Status | Accepted |
| Satisfies | [REQ-BOUNDED-PAGES](#01M7BF6HC3AE196XHCMXGKJXFS) |

- Each (snapshot, normalized request) result is computed once into a bounded result cache. Pages are slices of it.
- Byte budgets use per-record sizes computed once, never by re-serializing the page after each record.
- A cursor carries the key of its result ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)), which covers every layer and definition the result reads, so staleness is detected exactly.
- A read's result key covers the key of the layer it reads, every input of the request but the page size and the cursor, so a walk may change its page size, and the code, path, line and message of the inventory's diagnostics that a read of documents carries. The layer of `schema list` and `schema get` is the configuration and schema files as read; that of `get` of a code location, which builds no snapshot ([DES-CODE-READERS](../code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)), is the hash of the code file's contents, or the key of its scan when its language's grammar scanned it, and its key also covers the code, path, line and message of each diagnostic it returns; that of every other read, the document snapshot. A cursor of another result is stale. `get`'s cursor offset counts the bytes of the row values, then of the content, before its page.
- Internal consumers call evaluate-all APIs, never the public paged API in a loop.
- `get` returns a node larger than one page in consecutive portions: its metadata rows first, then its content cut at line boundaries, or at a character boundary inside a line or value longer than a page. The portions concatenate to the exact content.
- A row whose value is cut is `partial` on each page but the one that ends it; the parts of its value concatenate to it.
- A record of any other read that cannot fit one page fails the read with a diagnostic that names the record. Nothing is cut silently.

---

<a id="01M7DMT8FTCKV3J0TP5H3S3AFA"></a>

### Search and list requests

| Type | Design |
| --- | --- |
| ID | DES-SEARCH-REQUESTS |
| Status | Accepted |
| Satisfies | [REQ-SEARCH-ITEMS-AND-NARRATIVE](#01M7BF6HA7YSRRQ2BWS54AEGSG) |
| Satisfies | [REQ-SEARCH-RANKING](#01M7BF6HAHQJNEGTAV5A2AY9RX) |
| Satisfies | [REQ-LIST-ITEMS](#01M7BF6HBTZ6YD1XBDXRD4ZCC4) |

| Filter | Value | Matches |
| --- | --- | --- |
| `--type` | A type name | Items of that type |
| `--field` | `<key>=<value>` | Items with a row of that key and value |
| `--relation` | A forward or inverse relation name | Items that an edge of that relation leaves, for a forward name, or targets, for an inverse name |
| `--path` | A document or a directory, from the project root, read as `project validate` reads it ([DES-DIAGNOSTICS](../validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)) | Nodes in that document, or in a document below that directory |
| `--id` | An `ID` value or a MID | The items that have it |

- Every filter may repeat. Values of one filter are alternatives; different filters, and field filters of different keys, all apply.
- Keys, type and relation names, and `Status` and enum values match as labels do ([DES-METADATA-TABLE](../format.mara.md#01M7BF6GZNS6VM3T94VS9A5HTD)); any other value matches as written.
- A type, field, relation or ID filter matches no narrative unit.
- The query is one argument, read as words like the documents ([DES-SNAPSHOT](../engine.mara.md#01M7BF6GYM8NPPGY9RZRF38EGE)). A query without words matches every node.
- Within each group of [REQ-SEARCH-RANKING](#01M7BF6HAHQJNEGTAV5A2AY9RX), a node with more query words matched in its ID, title or a heading comes first. In the group of exact matches only exact ones count.
- The excerpt is the line of the node's first occurrence of a query word, else of a word within the allowed edits of one, as the token index records the words and where they start ([DES-SNAPSHOT](../engine.mara.md#01M7BF6GYM8NPPGY9RZRF38EGE)); without query words, the node's first line of text other than its anchor. It shows at most 240 characters of that line, from up to 60 before the word. A word in a code span that runs over several lines has the position of the code span's start, so its excerpt is the line where the span starts.

---

<a id="01M7DMT8GAJCAFTW0Y2939JFF1"></a>

### Read results

| Type | Design |
| --- | --- |
| ID | DES-READ-RESULTS |
| Status | Accepted |
| Satisfies | [REQ-CLI-MCP-PARITY](../hosts/index.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |
| Satisfies | [REQ-BOUNDED-PAGES](#01M7BF6HC3AE196XHCMXGKJXFS) |
| Satisfies | [REQ-UNAVAILABLE-NOT-PARTIAL](../engine.mara.md#01M7BF6GXSJVVASJ4C8ZGHM909) |

| Operation | Takes | Records |
| --- | --- | --- |
| `search` | A query, filters, `--limit`, `--cursor` | `results`: a node with `excerpt` (`line`, `text`) |
| `get` | A node, `--cursor` | None: `node`, then this page's `rows` (`key`, `value`, `line`, `partial`) and `content` |
| `relation list` | A node, `--relation`, `--limit`, `--cursor` | `connections`, after `node` |
| `item list` | Filters, `--limit`, `--cursor` | `items`: item nodes |
| `schema list` | `--limit`, `--cursor` | `declarations`: `kind`, `name`, `id_prefix` or `inverse`, `description` |
| `schema get` | A type name, or a relation's forward or inverse name; `--limit`, `--cursor` | `declarations`: each declaration of that name as loaded ([DES-SCHEMA-MODEL](../project.mara.md#01M7BF6HA0KG3R3AATWM1P2X1G)), a type with the `Status` values |

- A result is a JSON object of `format_version` (1), its operation's members, `diagnostics` ([DES-DIAGNOSTICS](../validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)), `diagnostics_total` and `next_cursor`, which is `null` on the last page. The human form prints the records, then those diagnostics and, when some are left out, how many with a pointer to `mara project validate`, then the next cursor.
- A node is `{kind: "item", path, line, id, mid, type, title}`, `{kind: "narrative", path, line, heading}` or `{kind: "document", path}`; a code location or file is `{kind: "code", path, line, declaration}` ([DES-CODE-MARKERS](../code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)), `line` null for a whole file and `declaration` null at the basic level. `get` adds an item's `revision`: the lowercase hexadecimal BLAKE3 hash of its region.
- A connection is `{kind, relation, direction, neighbour, source}`: `kind` is `relation` or `mention`, `relation` the name shown or `null`, `direction` `outgoing` or `incoming`, `neighbour` a node or `{kind: "unresolved", mid}`, `source` the `path` and `line` of the link.
- `relation list` lists an item's outgoing relations in row order and its mentions in source order, then the relations and mentions that target it in path and source order, then its code links in path and line order; a code file's or code line's code links in line order; a narrative unit's mentions; and every relation and mention written in a whole document, in source order. A link to a MID that several items have connects to each of them.
- A node is named by an ID, a MID, `<path>:<line>` or `<path>`; `get` and `relation list` take every one. An ID or MID that several items have names none: the read fails with their locations. Failure codes are those of [DES-OPERATIONS](../hosts/index.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS); `record-too-large` carries a diagnostic at the record's location.
- A read of documents carries the inventory's diagnostics ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)) on every page: the first of them in path and line order, as many as fit in 8,192 bytes of JSON, and `diagnostics_total`, how many there are. Its result is complete only when the total is 0; `project validate` pages every one. So every page of every read, with records or without, stays within the page's bytes.
