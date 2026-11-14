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

Search also matches a word within one edit of a query word of 4–7 characters, or two edits of a longer one. ID and MID values match only exactly.

---

<a id="01M7BF6HB1DC9AFHM890ERAJCK"></a>

### Read one node

| Type | Requirement |
| --- | --- |
| ID | REQ-READ-NODE |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`get` returns one item by ID or MID, or a document or narrative unit returned by search: its content, its metadata rows in source order and its location. It returns no neighbours.

---

<a id="01M7BF6HBCR1H0P6S8KWBV3DAS"></a>

### Follow direct connections

| Type | Requirement |
| --- | --- |
| ID | REQ-DIRECT-NEIGHBOURS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`related` returns the connections one hop from a node: outgoing relations, incoming relations under their inverse name, mentions and code links, each with its source location. It never expands a second hop.

---

<a id="01M7BF6HBTZ6YD1XBDXRD4ZCC4"></a>

### List items

| Type | Requirement |
| --- | --- |
| ID | REQ-LIST-ITEMS |
| Status | Accepted |
| Satisfies | [NEED-BOUNDED-RETRIEVAL](product.mara.md#01M7BF6H6GBKEHK5BQ8NDX7JG5) |

`item list` returns compact item summaries in path and source order, narrowed by the same filters as search.

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

A page holds 20 records unless the request asks for another number from 1 to 100, and at most 65,536 bytes. The numbers are those of 0.3.0 and provisional.

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
- Search returns a unit as `<path>:<line>`, the line where it starts.
- `get <path>:<line>` reads the unit or item that contains that line. `get <path>` reads the whole document.

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
- A cursor carries the snapshot key ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)), so staleness is detected exactly.
- Internal consumers call evaluate-all APIs, never the public paged API in a loop.

---

## Open questions

- Whether inspecting the authored occurrences of one relation stays its own operation (`relation get` in 0.3.0) or becomes part of `related`.
- How `get` pages a node whose content exceeds one page.
