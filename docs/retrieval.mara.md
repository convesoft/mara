# Retrieval

## Requirements

---

<a id="01M7BF6HA7YSRRQ2BWS54AEGSG"></a>

### Search items and narrative

| Type | Requirement |
| --- | --- |
| ID | REQ-SEARCH-ITEMS-AND-NARRATIVE |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Search returns the items and narrative whose own text contains every query word, each with its location and one excerpt. Filters by type, field value, relation, path and ID match exactly.

---

<a id="01M7BF6HAHQJNEGTAV5A2AY9RX"></a>

### Rank search results reproducibly

| Type | Requirement |
| --- | --- |
| ID | REQ-SEARCH-RANKING |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Results that match every word exactly precede results that need typo tolerance. Within each group a match in the ID, title or a heading outranks a match in the body or a field, and ties follow path and source order.

---

<a id="01M7BF6HAS8Y6FSG3BXXPTZNGP"></a>

### Tolerate small spelling errors

| Type | Requirement |
| --- | --- |
| ID | REQ-TYPO-TOLERANT-SEARCH |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

Search also matches a word within one edit of a query word of 4–7 characters, or two edits of a longer one. An edit inserts, deletes or substitutes one character, or swaps two adjacent characters that no other edit touches (optimal string alignment). A query word of more than 18 characters, and ID and MID values, match only exactly.

---

<a id="01M7BF6HB1DC9AFHM890ERAJCK"></a>

### Read one node

| Type | Requirement |
| --- | --- |
| ID | REQ-READ-NODE |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`get` returns one item by ID or MID, or a document or narrative unit returned by search: its content, its metadata rows in source order, its location and, for an item, its revision ([DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)). It returns no neighbours.

---

<a id="01M7BF6HBCR1H0P6S8KWBV3DAS"></a>

### Follow direct connections

| Type | Requirement |
| --- | --- |
| ID | REQ-DIRECT-NEIGHBOURS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`relation list` takes one node and returns the connections one hop from it: outgoing relations, incoming relations under their inverse name, mentions and code links, each with its source location. It narrows by relation name and never expands a second hop.

---

<a id="01M7BF6HBTZ6YD1XBDXRD4ZCC4"></a>

### List items

| Type | Requirement |
| --- | --- |
| ID | REQ-LIST-ITEMS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`item list` returns compact item summaries in path and source order, narrowed by the same filters as search. A summary holds the item's ID, MID, type, title, path and line.

---

<a id="01M7BF6HC3AE196XHCMXGKJXFS"></a>

### Every read is paged

| Type | Requirement |
| --- | --- |
| ID | REQ-BOUNDED-PAGES |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

A read returns at most a bounded number of records and bytes, with a cursor while more remain. Following the cursors yields each record exactly once. A cursor from a snapshot that has since changed is rejected as stale.

---

<a id="01M7CMW719S9H8FQKRFWWQ6NQK"></a>

### Page limits

| Type | Requirement |
| --- | --- |
| ID | REQ-PAGE-LIMITS |
| Status | Draft |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

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
- A cursor carries the key of its result ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)), which covers every layer and definition the result reads, so staleness is detected exactly.
- A read's result key covers the key of the layer it reads, every input of the request but the page size and the cursor, so a walk may change its page size, and the code, path, line and message of the inventory's diagnostics that a read of documents carries. The layer of `schema list` and `schema get` is the configuration and schema files as read; that of every other read, the document snapshot. A cursor of another result is stale. `get`'s cursor offset counts the bytes of the row values, then of the content, before its page.
- Internal consumers call evaluate-all APIs, never the public paged API in a loop.
- `get` returns a node larger than one page in consecutive portions: its metadata rows first, then its content cut at line boundaries, or at a character boundary inside a line or value longer than a page. The portions concatenate to the exact content.
- A row whose value is cut is `partial` on each page but the one that ends it; the parts of its value concatenate to it.
- A record of any other read that cannot fit one page fails the read with a diagnostic that names the record. Nothing is cut silently.
