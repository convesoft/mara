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

An edit that Mara refuses leaves every file byte-identical. Mara refuses an edit when any file it would write changed after the snapshot the edit was computed against.

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

<a id="01M7CXC2N8EZYVHGY7T2GPPB3X"></a>

### Mara's writers never overwrite each other

| Type | Requirement |
| --- | --- |
| ID | REQ-WRITERS-SERIALIZED |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

When two Mara processes edit the same project at the same time, neither edit is lost: the second one is computed after the first, or is refused.

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
| Satisfies | [REQ-WRITERS-SERIALIZED](#01M7CXC2N8EZYVHGY7T2GPPB3X) |

For each edit:

1. Take the project's write lock: an exclusive advisory lock on the `.mara` directory, held until the edit ends. Every Mara writer takes it: a direct CLI call, a daemon of any version, an embedded host.
2. Compute the edit, then check that every file it will write still has the hash recorded in the snapshot it was computed against. A mismatch refuses the edit.
3. For each file, in the order below: write a temporary file, check the target's hash once more, and rename the temporary file into place.
4. Re-derive only the changed files.

- A mismatch in step 3 after the first rename stops the edit and leaves an interrupted edit ([REQ-INTERRUPTED-EDIT-LOSES-NOTHING](#01M7BF6HJ3758Y06V61901ZSKK)).
- The lock binds only Mara. A save by another program between the last check and the rename is not detected.

Order across files:

| Operation | Order | Rerunning an interrupted operation |
| --- | --- | --- |
| Move | Destination file, then removal from the source file, then inbound link paths | Works while the item is in both files: the move skips the first step when the two copies are identical |
| Rename | Code markers that name the old ID, then the item's `ID` row, then inbound link text | Works while the old ID still resolves |

Fix rewrites the stale links an interruption leaves ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)).
