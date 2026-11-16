---
title: Review the decisions workers made in alpha.2
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-6k50w0
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.2 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.2 epic (mara-6k50w0) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf: what was decided, what it rests on, how to undo it. The decisions of 2026-11-15 were made under the maintainer's rules of that day (only findings that a realistic setup reaches block integration; exotic ones go to the backlog) and are listed here under the rule of 2026-11-16. The `## Decisions made` lists of the slice's issues are appended when the epic closes.

- 2026-11-15, mara-sjjsnm: the six findings of the third review of the writer's read-back guard (mara-6ex559) are exotic; the issue went to the backlog and nothing was fixed. Rests on ADR-READ-BACK-EVERY-EDIT, whose purpose (no edit silently loses or swallows an item) the operations built since then meet in review. Undo: pull mara-sjjsnm out of the backlog. Related backlog: mara-w8gafr.
- 2026-11-15, mara-hh81bw (measure the snapshot rebuild after one edit against the warm read budget) moved from alpha.4 to the backlog: low priority, REQ-TIME-BUDGETS is Draft, and mara-p27z9t times the warm read. Undo: give it mara-0mxw21 as parent again.
- 2026-11-15, mara-jnvzxr was done inside mara-esxqzv as a refusal: a move is refused when the moved item uses a link reference definition outside it. Rests on REQ-MOVE-ITEM (a move keeps content). Undo: mara-h8bc9x (carry the definitions with the item) replaces the refusal.
- 2026-11-15, mara-esxqzv: left to the backlog, with the limits stated in DES-MOVE-AND-RENAME: a move does not rewrite the moved item's links to plain files and images (mara-y9yz9c); definitions after an item's last block are not carried, so such a move is refused (mara-h8bc9x); a link title that repeats the destination is rewritten instead of the destination (mara-j30pfj). Rests on REQ-MOVE-ITEM: none loses data silently, validation reports each. Undo: pull the issue.
- 2026-11-15, mara-pvqpse: `project fix` adds an anchor only when nothing stands in the anchor's place; a paragraph of inline HTML there, such as an anchor with a lowercase MID, stays reported. The Accepted DES-ITEM-STRUCTURE was changed to say so. Rests on REQ-ONE-FIX-OPERATION and DES-MID-ANCHOR: a second anchor with a new MID would break every link that uses the old one. Undo: a new issue that repairs the damaged anchor in place.
- 2026-11-15, mara-pvqpse: left to the backlog: a stale mention whose destination uses backslash escapes or character references, or whose title repeats the destination, stays reported (mara-rhaycg; limit stated in DES-FIX-REPAIRS); fix can leave a `duplicate-row` when two rows name one target in different forms (mara-m1r77q; the reviewer called it reachable in ordinary projects, kept in the backlog because validation reports it and no data is lost). Undo: pull the issue.
- 2026-11-15, mara-w0xasr: `item update --clear-field` was kept from 0.3.0 instead of dropped. Rests on AGENTS.md (0.3.0 where the capability is kept) and REQ-UPDATE-ITEM. Undo: not needed; dropping it would be the maintainer's decision.
- 2026-11-15, mara-w0xasr: `relation remove` and `relation add` act only when the item's type is a source of the relation, so a field that shares a relation's name is left alone. Rests on DES-SCHEMA-CHECKS and the maintainer's alpha.1 answer that such a row is `undeclared-key` and no edge. Clearing a field whose rows start a hand-written table is refused (backlog mara-560e3c). Undo: pull the issue.
- 2026-11-15, mara-w0xasr: `item create` reports a bad destination with the codes of `item move`, `not-an-input` and `invalid-position`; its own `invalid-location` is gone. Rests on DES-OPERATIONS (one code per problem). Undo: a new issue; no release has carried the codes yet.
- 2026-11-15, mara-wjy3sh: the converter checks its output with its own read-back (every item, row and body reads as written) instead of the writer's `ReadBack`. Rests on ADR-READ-BACK-EVERY-EDIT's purpose. YAML merge keys are not applied and are reported (backlog mara-gsdycs; limit stated in DES-CONVERSION-MAPPING). New dependency `granit-parser`, the YAML parser 0.3.0 used, in the `mara` crate only. Rests on REQ-CONVERT-DIRECTIVE-CORPUS. Undo: issues.
- 2026-11-15, mara-wjy3sh: DES-INPUTS-AND-KEYS gained the clause on a converted schema left by an earlier run. Rests on the maintainer's amendment of the `project convert` row of VER-WORK-COUNTERS that day. Undo: with that row.
- Decided by the maintainer during the slice and recorded word for word in the issues named: integrate mara-6ex559 with six open findings (mara-sjjsnm); amend the `project convert` row of VER-WORK-COUNTERS (mara-wjy3sh); build the MCP server on `rmcp` (mara-8vqk4a).
