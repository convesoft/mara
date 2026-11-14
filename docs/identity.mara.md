# Identity and links

## Requirements

---

<a id="01M7BF6H1AN6G9GP3DV62342WJ"></a>

### The MID survives every edit

| Type | Requirement |
| --- | --- |
| ID | REQ-STABLE-IDENTITY |
| Status | Accepted |
| Satisfies | [NEED-STRUCTURED-KNOWLEDGE](product.mara.md#01M7BF6H61ST5R4855YZKCNJHC) |

An item's MID never changes when its title, ID or file changes, and every link that carries the MID keeps resolving to the item.

---

## Design

---

<a id="01M7BF6H1MB5RR3RS43B3CJHAR"></a>

### MID and its anchor

| Type | Design |
| --- | --- |
| ID | DES-MID-ANCHOR |
| Status | Accepted |
| Satisfies | [REQ-STABLE-IDENTITY](#01M7BF6H1AN6G9GP3DV62342WJ) |

- A MID is a canonical uppercase 26-character ULID, unique in the project and stored only in an HTML anchor: `<a id="<MID>"></a>`.
- The anchor is its own paragraph, followed by a blank line and the item heading. An anchor merged into another paragraph is an error.
- Anchors and links keep the uppercase MID.

---

<a id="01M7BF6H1ZBTKWTF9ZXEJ8EBGS"></a>

### Human-readable IDs

| Type | Design |
| --- | --- |
| ID | DES-ITEM-IDS |
| Status | Accepted |
| Satisfies | [REQ-STABLE-IDENTITY](#01M7BF6H1AN6G9GP3DV62342WJ) |

- Every item of every type has exactly one `ID` row, unique in the project. The schema cannot make IDs optional.
- An ID matches `[A-Z][A-Z0-9]*(-[A-Z0-9]+)+` and starts with its type's prefix. Prefer a readable slug: `REQ-PHISHING-RESISTANT-AUTH`.
- The ID is the relation link text, the CLI and MCP handle, and the grep target. It is a renameable alias; the MID is the identity.

---

<a id="01M7BF6H29JGHG65SV91K6D11V"></a>

### Relations and mentions

| Type | Design |
| --- | --- |
| ID | DES-RELATION-LINKS |
| Status | Accepted |
| Satisfies | [REQ-SOURCE-IS-PUBLISHED-FORM](format.mara.md#01M7BF6GYWE2DT7WSXQH2QPNPA) |
| Satisfies | [REQ-STABLE-IDENTITY](#01M7BF6H1AN6G9GP3DV62342WJ) |

- A relation is a metadata row on the source item, keyed by the forward relation name. Inverse names (`Satisfied by`) appear only in reading and navigation.
- The value is a Markdown link: `[<ID>](<relative-path>#<MID>)`, or `[<ID>](#<MID>)` within the same file.
- The fragment MID is the identity. The path is the file that currently contains that MID, relative to the linking file. The link text is the target's current ID.
- A stale path, stale link text or a bare-ID value is a diagnostic.
- Any body or narrative link that is not an absolute URL and whose fragment has MID form is a mention. Its text is free; its target and path are checked like a relation link's. Typed relations exist only in the metadata table.

---

## Evidence

---

<a id="01M7BF6H2MHAVK1Z5HCXR1V289"></a>

### GitHub resolves uppercase MID fragments

| Type | Evidence |
| --- | --- |
| ID | EVD-GITHUB-FRAGMENT-MATCHING |
| Status | Accepted |
| Supports | [DES-MID-ANCHOR](#01M7BF6H1MB5RR3RS43B3CJHAR) |

Checked on github.com on 2026-11-13 for same-file, cross-file and table-cell links. GitHub renders the anchor as `id="user-content-<lowercase MID>"` and keeps link hrefs as written. Its page script matches fragments case-insensitively on page load and on in-page clicks.
