# Write safety

## Requirements

---

<a id="01M7BF6HHH01T5XJNAF7DM433R"></a>

### A write touches only what it must

| Type | Requirement |
| --- | --- |
| ID | REQ-MINIMAL-REWRITE |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

A write changes only the spans its operation requires. All other bytes, including the author's Markdown formatting and line endings, and the file's permissions are preserved.

---

<a id="01M7BF6HHTMM6D6QAF0BHFBCE6"></a>

### A refused edit changes nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-REFUSED-EDIT-CHANGES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit that is refused, or whose target files changed after the snapshot it was computed against, leaves every file byte-identical.

---

<a id="01M7BF6HJ3758Y06V61901ZSKK"></a>

### An interrupted edit loses nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-INTERRUPTED-EDIT-LOSES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit interrupted between files never loses an item. It leaves at worst a duplicate item, stale link paths, stale link text or code markers whose ID does not resolve yet, each reported as a diagnostic.

---

## Design

---

<a id="01M7BF6HJA1E3SE8WJH77FGP17"></a>

### Write procedure

| Type | Design |
| --- | --- |
| ID | DES-WRITE-PROCEDURE |
| Status | Accepted |
| Satisfies | [REQ-REFUSED-EDIT-CHANGES-NOTHING](#01M7BF6HHTMM6D6QAF0BHFBCE6) |
| Satisfies | [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](#01M7BF6HJ3758Y06V61901ZSKK) |

For each file:

1. Check that the file still has the hash recorded in the snapshot the edit was computed against.
2. Write a temporary file and rename it into place.
3. Re-derive only the changed files.

Order across files:

| Operation | Order | Rerunning an interrupted operation |
| --- | --- | --- |
| Move | Destination file, then removal from the source file, then inbound link paths | Works while the item is in both files: the move skips the first step when the two copies are identical |
| Rename | Code markers that name the old ID, then the item's `ID` row, then inbound link text | Works while the old ID still resolves |

Fix rewrites the stale links an interruption leaves ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)).
