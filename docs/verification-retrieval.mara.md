# Retrieval verification

---

<a id="01M7D3NAS8KAN402KHJAKQ8HZ6"></a>

### Search a corpus with planted words

| Type | Verification |
| --- | --- |
| ID | VER-SEARCH-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-SEARCH-ITEMS-AND-NARRATIVE](retrieval.mara.md#01M7BF6HA7YSRRQ2BWS54AEGSG) |
| Verifies | [REQ-SEARCH-RANKING](retrieval.mara.md#01M7BF6HAHQJNEGTAV5A2AY9RX) |
| Verifies | [REQ-TYPO-TOLERANT-SEARCH](retrieval.mara.md#01M7BF6HAS8Y6FSG3BXXPTZNGP) |

Rust integration tests run `search` with the real CLI on a disposable project whose items and narrative hold planted words.

| Case | Expectation |
| --- | --- |
| Two query words, both in one item's body and one in another's | Only the first item, with its location and one excerpt |
| A word only in a narrative unit | That unit as `<path>:<line>` ([DES-NARRATIVE-UNITS](retrieval.mara.md#01M7CMW710P9V73WXFQ15PBYBS)) |
| A word only in the body of an item that another item links to | Not the linking item |
| A filter by type, field value, relation, path or ID | Exactly the results that match it; a filter value one edit away from a real one matches nothing |
| Exact and one-edit matches in titles, headings, bodies and fields, and exact matches in IDs, across several files | Exact matches first; within each group, ID, title and heading matches before body and field matches; ties in path and source order |
| A query word one edit away from a corpus word, at 3, 4 and 7 characters; two edits away, at 7 and 8; three edits away, at 12 | Matches at 4 and 7 with one edit and at 8 with two, and no other |
| An ID or MID one edit away | No match |

---

<a id="01M7D3NASEYQGVRAFXRGN112AF"></a>

### Read each kind of node

| Type | Verification |
| --- | --- |
| ID | VER-NODE-READS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-READ-NODE](retrieval.mara.md#01M7BF6HB1DC9AFHM890ERAJCK) |

Rust integration tests run `get` with the real CLI on a disposable project.

| Case | Expectation |
| --- | --- |
| `get` by ID and by MID of the same item | The same result: content, metadata rows in source order, location and revision, and nothing of the items it links to |
| `get <path>:<line>` of a line inside an item, and of a line inside a narrative unit | That item; that unit |
| `get <path>` | The whole document |
| The item's revision after an edit inside the item, and after an edit elsewhere in its file | Changed; unchanged |

---

<a id="01M7D3NASN0KJ0REKBPXNRGGMG"></a>

### List one hop around a node

| Type | Verification |
| --- | --- |
| ID | VER-ONE-HOP |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-DIRECT-NEIGHBOURS](retrieval.mara.md#01M7BF6HBCR1H0P6S8KWBV3DAS) |

Rust integration tests run `relation list` with the real CLI on a disposable project. Item A has outgoing relations, an incoming relation from item B, mentions to and from it and a code link; B has relations of its own.

| Case | Expectation |
| --- | --- |
| `relation list` of A | Each outgoing relation, B's under its inverse name, each mention and the code link, each with its source location; none of B's other relations |
| `relation list` of A narrowed by one relation name | Only that relation's connections |
| `relation list` of a narrative unit that mentions A | That mention |

---

<a id="01M7D3NASVJ4HCJ92180KKZJSP"></a>

### List items under each filter

| Type | Verification |
| --- | --- |
| ID | VER-ITEM-LIST |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-LIST-ITEMS](retrieval.mara.md#01M7BF6HBTZ6YD1XBDXRD4ZCC4) |

Rust integration tests run `item list` with the real CLI on a disposable project with items in several documents.

| Case | Expectation |
| --- | --- |
| No filter | Every item once, in path and source order |
| An item whose body is 20,000 bytes and ends with a word found nowhere else | The listing omits the full body: that word is absent |
| Each filter case of [VER-SEARCH-CASES](#01M7D3NAS8KAN402KHJAKQ8HZ6) | Exactly the items that match the filter, in path and source order |

---

<a id="01M7D3NAT18PH9EBV4N6ANCWVR"></a>

### Follow cursors to the end

| Type | Verification |
| --- | --- |
| ID | VER-CURSOR-WALK |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-BOUNDED-PAGES](retrieval.mara.md#01M7BF6HC3AE196XHCMXGKJXFS) |

Rust integration tests run each read operation with the real CLI on a disposable project with more results than one page holds.

| Case | Expectation |
| --- | --- |
| Following every cursor from the first page | Each record exactly once; every page within the record and byte limits; no cursor on the last page |
| A cursor used after a document edit | Rejected as stale |
| A cursor used after a change to an undeclared file | Accepted |
| `get` of an item larger than one page, with a line longer than a page | Consecutive portions, metadata rows first, that concatenate to the exact content |
| Any other read with a record larger than one page | Fails with a diagnostic that names the record |

---

<a id="01M7D3NAT895Q9KKDYKDQPJADJ"></a>

### Check the page limits

| Type | Verification |
| --- | --- |
| ID | VER-PAGE-LIMITS |
| Status | Draft |
| Methods | Test |
| Verifies | [REQ-PAGE-LIMITS](retrieval.mara.md#01M7CMW719S9H8FQKRFWWQ6NQK) |

Rust integration tests run each read operation with the real CLI on a disposable project with more than 100 results.

| Case | Expectation |
| --- | --- |
| No limit requested | 20 records per page |
| A limit of 1, and of 100 | That many records per page |
| Records larger than 65,536 bytes together | No page's JSON exceeds 65,536 bytes |

---

<a id="01M7D3NAS2JBPHJ6ZJNQXQSKNX"></a>

### Compare schema inspection with the schema file

| Type | Verification |
| --- | --- |
| ID | VER-SCHEMA-INSPECTION |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-SCHEMA-INSPECTION](project.mara.md#01M7BF6H9F8E7RB3XBSE0EKAR3) |

Rust integration tests run `schema list` and `schema get` with the real CLI on a disposable project whose schema declares fields of both kinds and relations with every constraint.

| Case | Expectation |
| --- | --- |
| `schema list` | Every declared type and relation, and nothing else |
| `schema get` of a type | Its description, ID prefix and fields, each with kind, allowed values, required and repeatable, as declared |
| `schema get` of a relation | Its forward and inverse names, description, target types per source type and constraints, as declared |
