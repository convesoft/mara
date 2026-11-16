---
title: A hand-written item without a separator before it is read as the previous item's body, with no diagnostic
status: todo
type: bug
discovered_from: mara-18193r
created_at: 2026-11-16T10:50:38Z
---

Goal: an item written by hand directly after another item, without the `---` line before it, is reported, so that it does not silently drop out of the project.

Context: found while writing the agent guide (mara-18193r). In a disposable project with an item at the end of `docs/a.mara.md`, append by hand, without `---`:

```
## Passkey verification

| Type | Verification |
| --- | --- |
| ID | VER-PASSKEY |
| Status | Draft |
| Verifies | REQ-PHISHING-RESISTANT-AUTH |

Text.
```

`mara project validate` reports `valid: 0 errors, 0 warnings`, `mara project fix` finds nothing, and `item move VER-PASSKEY ...` fails with `unknown-item`. With `---` before the heading, validation reports `incomplete-item` and one `project fix` repairs it. The rules of "Documents and regions" in docs/format.mara.md ("An item ends only at a thematic break or end of file. Headings never end an item") make the text body of the item before, so no rule covers it. Agents append items this way; the guide (mara-18193r) tells them to put `---` first, but a forgotten separator loses the item without a word.

Changing it adds a diagnostic (and perhaps a `project fix` repair) to Accepted items in docs/format.mara.md and docs/authoring-requests.mara.md (DES-FIX-REPAIRS): a maintainer decision.

- [x] Decide (decided by the orchestrator on 2026-11-16, below): a heading followed by a table with a `Type` row inside an item body is reported (for example `missing-separator` at the heading), and whether `project fix` inserts the `---`
- [ ] Implement it with its validation and fix tests

Decided by the orchestrator on the maintainer's behalf on 2026-11-16, resting on REQ-REPORT-EVERY-PROBLEM and REQ-ONE-FIX-OPERATION: the problem gets a diagnostic, and `project fix` repairs it. The worker of this issue designs the rule so that an ordinary table with a `Type` column in an item's body is not mistaken for an item. Until then the agent guide (`mara prime`, mara-18193r) tells agents to write the `---` before the item first.

Decided by the orchestrator on 2026-11-16, after the review of mara-18193r showed that the same silent case also happens to a first item written after a title and introduction: this issue covers every heading followed by a `Type` table that is not the first block of its region, not only an item after another item, and it starts as soon as mara-18193r is integrated.
