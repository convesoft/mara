---
title: "Read-back guard: close the six findings of re-check 3 on the item writer"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-6ex559
  - mara-jnvzxr
discovered_from: mara-6ex559
created_at: 2026-11-15T14:49:59Z
---

Goal: the writer's read-back guard refuses every edit after which bytes the edit did not write read differently, refuses no valid edit, and does near-linear work.

Context: DES-ITEM-WRITER and ADR-READ-BACK-EVERY-EDIT in docs/authoring.mara.md; REQ-NEAR-LINEAR-SCALING, VER-SCALING, VER-WORK-COUNTERS and DES-PERFORMANCE-HARNESS in docs/verification.mara.md; code in `crates/mara-engine/src/write.rs`, `crates/mara-engine/src/write/read_back.rs`, `crates/mara-engine/src/document.rs`; history in mara-6ex559 (archived: "Review round 1" to "Review round 3").

## Why this issue exists

mara-6ex559 was integrated as `8cdae8d` with six open review findings, by the maintainer's decision. Three review rounds had each ended with the same defect, "an accepted edit changes what bytes it did not write read as", each time in new cases after the reported ones were fixed.

Decided by the maintainer, 2026-11-15. Question: "The third fix round of mara-6ex559 (item writer) ended with the same defect again: the reviewer still found accepted edits that change what untouched text reads as. Last round's cases are fixed; the six new findings are narrower. Three are that defect (an edit with a zero-length change; an edit that turns the start of a paragraph into an HTML comment; footnote labels that differ only by letter case, such as ß and SS). One valid edit is refused (rewriting a footnote definition). Two are speed (two insert/remove shapes are still quadratic, and the new timing test does not catch a planted quadratic lookup). Every other check passes, and all five waiting alpha.2 issues depend on this branch. How do you want to continue?" Chosen option: "Integrate, finish in parallel (Recommended) — I merge the branch as it is and file the six findings as one issue that must close, reviewed, before the alpha.2 slice can close. A fresh worker takes that issue while five workers start the operations that have been waiting. Nothing is tagged with the gaps open. Risk: the operations are built on a check with known gaps and may need small changes when it is finished; this skips my review gate once."

The rule itself was decided by the maintainer one round earlier and still binds: "Third round, stricter rule (Recommended) — Same worker. Instead of patching the two cases, the check skips only the exact bytes the edit changes and compares everything else in the document, with no shortcut for an item or narrative that the edit merely touches. A new reviewer re-checks. If that round ends with this defect again, I stop and come back to you." The cost also stays as decided: one parse per distinct new content, before the renames (ADR-READ-BACK-EVERY-EDIT).

Orchestrator's note, not a decision: three rounds of fixing the reported cases did not converge. Look for one principle that makes the family impossible (for example, every byte the edit does not write keeps its role in the parse) before patching the six cases one by one. The operations (mara-w0xasr, mara-esxqzv, mara-pvqpse, mara-sj9jfq, mara-wjy3sh) are being built on `ReadBack::edit`, `Splice` and `Expect` at the same time: keep that interface, or report the change so their workers can be told.

## Findings of re-check 3 (reviewer's words; line numbers as of `8cdae8d`)

D1 — A zero-byte splice exempts an untouched reference. `read_back.rs:124`, `read_back.rs:648`. Document bytes `"[x][r]\n\n[r]: /a\n"`. Apply two `Expect::Bytes` splices: `1..1 → ""` and `13..15 → "/b"`. The edit is accepted. The unchanged `[x][r]` now targets `/b`. `crosses` treats the empty splice as touching the reference, so both reference comparisons skip it. It should refuse with `unreadable-edit`: the first splice writes no bytes.

D2 — Touching a block exempts its untouched contents. `read_back.rs:517`. Document bytes `"A\nUntouched text.\n"`. Apply `Expect::Bytes`, replacing `0..1` with `"<!--"`. The edit is accepted. `Untouched text.` becomes HTML-comment content and disappears from the document's search words. Its bytes were outside the splice. The old block is skipped because it overlaps the edit; the new block is skipped because it contains written text. The same bypass works inside an item's body. It should refuse or require the swallowed text to be included in the splice.

D3 — Rewriting a footnote definition falsely counts as changing its target. `read_back.rs:493`. Document bytes `"A[^n].\n\n[^n]: Note.\n"`. Apply `Expect::Bytes`, replacing `8..19` with `"[^n]:  Note."`. This only adds formatting whitespace. The first definition, its label and rendered content remain the same, but the edit is refused: "`[^n]` in it would refer to another footnote". Even replacing `8..9` with the identical `"["` is refused. `same_reading` assumes a definition is different whenever its start lies inside written text. An item-body replacement containing this definition produces the same false refusal. Such edits should pass without rewriting the outside reference.

D4 — Batch insert/remove still have quadratic paths. `write.rs:330`, `write.rs:389`, `write.rs:592`. Two reproductions, each producing an accepted batch: insert one ordinary item at each blank-line position in `"# Page\n\n" + "\n".repeat(n) + "Tail.\n"` (every call rescans the shared blank gap); remove all `n` ordinary items after front matter containing `n` lines (every call reparses the same front matter). Release measurements of splice construction at 1,000 / 2,000 / 4,000: insert across blank gap 4.40 / 16.29 / 62.02 ms; remove after front matter 6.81 / 27.19 / 110.61 ms. Input and output grow linearly; construction approaches 4× per doubling.

D5 — The timing test passes a quadratic guard and can fail from scheduling delays. `write/tests.rs:1363`, `reads_back_many_splices_in_time_proportional_to_the_document`. Replacing `self.lines.partition_point(|&start| start <= offset)` with `self.lines.iter().take_while(|&&start| start <= offset).count()` restores quadratic work across many removals, and the test still passes. It measures splice construction once with `Instant`, against the minimum parse time, so descheduling during that single sample can fail it without a code fault. Its fixed-size 20× comparison differs from the documented release-only CPU-time growth measurements, corpus sizes, medians and sampling procedure.

D6 — Footnote labels use lowercase instead of the parser's Unicode case folding. `document.rs:409`. Document bytes `"A[^SS].\n\n[^ß]: First.\n\n[^ss]: Second.\n"`. The Markdown parser considers `SS` and `ß` equivalent. The new lowercase-keyed map instead records the later `ss` definition as the reference's first definition. Apply `Expect::Bytes`, deleting `9..24`, which removes the first definition. The edit is accepted: the guard incorrectly believes the reference already used the surviving definition. It should refuse the change to the untouched reference's first matching definition. The inverse also occurs: starting with only the `ß` definition, inserting an item containing a later `ss` definition is falsely refused.

Passed in re-check 3 and not to be re-opened: the D1/D2 cases of re-check 2; 144 sequences of create, update, relation add/remove, move and delete across CRLF, lone CR, BOM, front matter, missing final newline, reference links, footnotes, HTML, nested lists and quotes and body tables; the allowance for deleting an unused first definition; the parser's one event pass, no I/O, the `wasm32` build; the docs and the cold write row.

## Tasks

- [ ] D1, D2 and D6: no accepted edit changes what bytes it did not write read as (items, titles, rows, values, links and their targets, mentions, images, footnotes, headings, front matter, search words). Each with a test that fails without the fix
- [ ] D3 and the inverse of D6: valid edits pass
- [ ] D4: `insert`, `remove` and the guard are near-linear for many splices in one edit
- [ ] D5: the wall-clock test is replaced by tests that fail on a planted quadratic path and cannot fail without a fault, in the way docs/verification.mara.md prescribes (work counters, or the release `performance` harness)
- [ ] DES-ITEM-WRITER and ADR-READ-BACK-EVERY-EDIT state the rule as built

## Moved to the backlog, 2026-11-15

The maintainer amended the orchestration rules on 2026-11-15: a finding is fixed now only if a realistic setup reaches it (documents and projects as users actually write them, and the documented CLI and MCP usage); exotic findings go to the backlog, which is looked at after the first release and in which nothing starts without the maintainer. This replaces the decision above that this issue must close before the alpha.2 slice can close. The worker that had started was stopped before it changed anything.

The orchestrator's triage of the six findings, all exotic:

- D1 needs a zero-length splice next to a change of a link reference definition. No operation emits a zero-length splice.
- D2 needs an edit that writes an unclosed `<!--` at the start of a paragraph and leaves the rest of the paragraph unwritten. The operations replace a whole title, row value or body.
- D3 needs a footnote that is defined inside the bytes an edit writes and used outside them, for example a body update of the item that holds the page's footnote definitions. The result is a refusal with a diagnostic, not lost data.
- D4 needs as many blank lines in one gap, or as many front matter lines, as the edit has insertions or removals.
- D5: the timing test has a 20× threshold against measured ratios of 0.1 to 0.9, and no false failure has been seen. If it ever fails without a fault, pull this finding back.
- D6 needs two footnote labels that differ only by Unicode case folding, such as `ß` and `SS`.
