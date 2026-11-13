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

Creating an item writes a complete item with a newly generated MID into a named document, at its end or at a given line outside every existing item. The caller supplies an ID or accepts one proposed from the type prefix and a title slug. A caller never supplies a MID.

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

Renaming an ID keeps the MID and rewrites the link text of every inbound link. It leaves no alias: the old ID no longer resolves.

---

<a id="01M7BF6GMX0QVD78B1ZTMY7PKF"></a>

### Delete an item

| Type | Requirement |
| --- | --- |
| ID | REQ-DELETE-ITEM |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Deleting an item is refused while a relation or mention in another item or in narrative targets its MID. The refusal lists each such location.

---

<a id="01M7BF6GN8DJ66NPBMQK83H9PW"></a>

### Add and remove relations

| Type | Requirement |
| --- | --- |
| ID | REQ-EDIT-RELATIONS |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

Adding a relation writes one canonical link row on the source item and is refused when the schema does not allow that relation between the two types. Removing a relation deletes that row.

---

<a id="01M7BF6GNPRRHNP2TRX7MKZRRH"></a>

### One fix operation repairs source

| Type | Requirement |
| --- | --- |
| ID | REQ-ONE-FIX-OPERATION |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

A single explicit fix operation repairs incomplete items, bare-ID relation values, stale link paths and link text, and malformed item formatting. No other operation repairs source.

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

---

## Open questions

- Whether adding a relation accepts the inverse name and writes the row on the other item.
