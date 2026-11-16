---
title: Write the Verification items for the alpha.5 requirements
status: done
type: task
priority: high
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-16T16:54:03Z
---

Goal: every requirement that the alpha.5 slice delivers has an accepted Verification item: the requirements that the issues of mara-779tms name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [x] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [x] One Verification item for each, or one shared by requirements that a single check covers
- [x] The test items in those issues point at the Verification items instead of restating them

Started on 2026-11-16 before the alpha.4 epic closed, by the orchestrator's decision under the maintainer's rule of that day: this issue writes documents only, and code links do not build on the daemon. The alpha.5 epic stays blocked as it is.

## Summary of Changes

The slice delivers these requirements. Each now has a Verification item:

| Requirement | Where the slice names it | Verification |
| --- | --- | --- |
| REQ-CODE-MARKER-LINKS | mara-yhxq5x, mara-s9gak4; DES-CODE-MARKERS, DES-CODE-READERS | VER-CODE-MARKERS, VER-GRAMMAR-MARKERS (new) |
| REQ-ONLY-CODE-ADD-DOWNLOADS | mara-h0hxa3; DES-GRAMMAR-ASSETS | VER-CODE-ADD, and the fallback cases of VER-GRAMMAR-MARKERS (new) |
| REQ-DIRECT-NEIGHBOURS | mara-d005q9 | VER-ONE-HOP (unchanged, its code-link case); the code location's end in VER-CODE-MARKERS |
| REQ-RENAME-ID, REQ-MINIMAL-REWRITE | mara-yhxq5x; DES-WRITE-PROCEDURE | VER-CODE-EDITS (new); VER-RENAME-ID's code row points at it |
| REQ-INTERRUPTED-EDIT-LOSES-NOTHING, REQ-REFUSED-EDIT-CHANGES-NOTHING | DES-WRITE-PROCEDURE | VER-INTERRUPTED-EDITS (code rows made concrete), VER-REFUSED-EDITS, with a code file as F in VER-CODE-EDITS |
| REQ-DELETE-ITEM | mara-yhxq5x | VER-DELETE-ITEM (code row completed) |
| REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED | mara-yhxq5x; DES-FACT-BRIDGE | VER-CODE-FACTS (new) |
| REQ-TEMPLATE-ACCEPTANCE-RULES, REQ-TEMPLATE-GATES-PROVE-READINESS | mara-yhxq5x | VER-TEMPLATE-CASES, four code rows from alpha.5 |
| REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING | mara-yhxq5x; DES-CODE-READERS, DES-INPUTS-AND-KEYS | VER-CODE-WORK-COUNTERS (new): cold rows, daemon warm rows for code inputs, hub project with markers |
| REQ-CLI-MCP-PARITY, REQ-SAME-RESULT-ON-EVERY-HOST | operations this slice extends | Through `mara mcp` and the daemon in each new item, as VER-CLI-MCP-PARITY and VER-HOST-COMPARISON require |
| REQ-AGENT-GUIDE | DES-AGENT-GUIDE's name comparison | VER-AGENT-GUIDE, unchanged; `code add` appears in neither rendering |

Not in this slice: REQ-UNAVAILABLE-NOT-PARTIAL for grammar-level code links in the browser (mara-jptfbq, alpha.6).

- New pages docs/verification-code.mara.md and docs/verification-code-use.mara.md, listed in docs/index.mara.md.
- DES-CODE-MARKERS, DES-CODE-READERS, DES-GRAMMAR-ASSETS, DES-SCHEMA-CHECKS and DES-AGENT-GUIDE hold the decisions below.
- Test items of mara-yhxq5x, mara-s9gak4 and mara-h0hxa3 now name the Verification items.
- Filed mara-0x04mn: the agent guide does not teach markers.

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked`, `project validate` (valid: 0 errors, 0 warnings), the release `performance` test (16 passed, 2 ignored), the `wasm32` build of `mara-engine` and `zban check`: all exit 0. New pages are 95 and 89 lines.

## Decisions made

- Marker form (DES-CODE-MARKERS), changed by the review (finding 3): a marker is a comment whose text is the marker, on its own line or after code on the same line, as in 0.3.0: the comment start, `@mara`, a forward relation name and an ID, and nothing after them but white space. `@mara` later in a comment's text is no marker. A JSDoc ` * @mara …` line works when `*` is a prefix.
- Comment start at the basic level (review, finding 3): the first configured prefix on a line that stands at the line's start or after a space or tab. So `"// @mara …"` holds no comment, while `" // @mara …"` is read as one and is `invalid-marker`, because text follows the ID. At grammar level a comment starts where its node starts, or at a prefix that begins a later line of a block comment, and text in a string is never a comment. An inline marker belongs to the innermost declaration that contains the start of its line, else to the file.
- `item rename` fails with `io-error` at a declared code file it cannot read, before writing anything (DES-CODE-READERS; review, finding 1).
- A prefix followed by `@mara` without one declared relation and one ID-form word is `invalid-marker`. An ID no item has is `unresolved-marker`, and a relation that does not lead from `code` to the target's type is `disallowed-target`. All three are errors at the marker, without an item ID or MID (DES-SCHEMA-CHECKS rows added).
- Code files are read as bytes, in LF-ended lines, so a file that is not UTF-8 still links and a rename changes only the ID bytes.
- At grammar level the same rule reads only lines inside comment nodes; a marker-like line in a string links at basic level only. The owning declaration follows 0.3.0: the declaration the comment precedes, else the innermost one that contains the marker, else the file. The location line is the marker's line at basic level, and at grammar level the declaration's first line, or 1 for the file. The source is always the marker's line.
- One code link per marker; a marker naming an ID that several items have links to each, as MID links do.
- `get` and `relation list` of `<path>:<line>` in a code file name the line at basic level and the owning declaration at grammar level; `<path>` names the whole file.
- `source_complete("code")` (DES-CODE-READERS): every declared code file read and scanned at either level, a grammar fallback included. A project with no code language has read all its code, so the engineering `verified` gate fails a requirement without an implementation instead of staying unavailable. An `io-error` code file leaves code incomplete. A program uses code links when it, or a file it includes, names the code-link relation.
- Grammars (DES-GRAMMAR-ASSETS): `code add` writes the configuration through the write procedure. A missing, mismatched or unloadable grammar is `unavailable-grammar`, an error at the pinning line that names `mara code add`, since DES-DIAGNOSTICS makes every built-in failure an error. A grammar runs as WebAssembly in tree-sitter's WASM store inside Mara's process, and `tags.scm` is a query.
- Provisional, decided by the orchestrator on the maintainer's behalf, for the maintainer to confirm: the grammar download trust policy, ADR-GRAMMAR-TRUST-ON-FIRST-USE (Draft) beside DES-GRAMMAR-ASSETS. Catalog entries carry their checksums. A repository the user gives is trusted on first use: the checksum of its fetched files and the repository are pinned, and every later fetch and load is checked against the pin. It must be `https://`, or `http://` only to a loopback host. Rejected: no pin; a required checksum argument for every repository outside the catalog; plain HTTP anywhere.
- Decided by the orchestrator on the maintainer's behalf: a missing grammar is the error `unavailable-grammar`, so `project validate` fails on a fresh machine until `mara code add` has run. Rests on DES-DIAGNOSTICS and REQ-ONLY-CODE-ADD-DOWNLOADS; the diagnostic names the fix. Undo: allow a warning for this one case.
- Decided by the orchestrator on the maintainer's behalf: a project with no code language fails the template's `verified` gate instead of reporting it unavailable. Rests on the gate's design: code counts as completely read, and nothing implements the requirement. Undo: change the gate design.
- Decided by the orchestrator on the maintainer's behalf (review, finding 3): inline marker comments stay supported, as the docs and then 0.3.0 order requires; dropping a capability that users see is not a worker's decision.
- `code add` is in neither rendering of the agent guide, like `prime` and `mcp` (DES-AGENT-GUIDE): it is no MCP tool.
- No read-back scan of rewritten code files: a rename replaces only an ASCII ID token inside a recognized marker line. VER-CODE-WORK-COUNTERS counts no code scan after the edit.
- "Reads no code" is asserted as the counts of the same case with no code language declared, since the counters do not tell code reads from document reads.
- Warm rows for code inputs live in VER-CODE-WORK-COUNTERS, to stay clear of mara-bx63yb's edits of VER-WORK-COUNTERS; that item's existing "One code file edited" row can only run from alpha.5.
- ABI mismatch is verified by inspection: no released grammar has an ABI outside the runtime's range, so a truncated module tests the same fallback. The real catalog is checked by an ignored test run before each release.

## Rejected approaches

- A network work counter, `strace` or a network namespace to show that no other operation connects. The first changes the stats format; the others are tools or privileges the docs do not provide. A loopback repository that counts connections tests the realistic failure, fetching a missing grammar, with the standard library.
- Fixing the JSON spelling of code nodes or the code-link relation's name in this issue. The cases name the values; mara-yhxq5x spells them in DES-READ-RESULTS and DES-FACT-BRIDGE.

## Review

One review of `b5c1b6b` found coverage complete, the form fine and no new test means needed. It reported five findings; the orchestrator decided each:

1. VER-CODE-EDITS missed a code file that cannot be read when the rename is planned. Added the case: the rename fails with `io-error` naming the file, before writing anything, and every file is unchanged. DES-CODE-READERS states the rule.
2. VER-CODE-EDITS ran at the basic level only. The rename cases now also run with the grammars installed, with markers before declarations and marker-like lines in raw strings; only the ID bytes of actual markers change.
3. DES-CODE-MARKERS had dropped inline marker comments; they stay supported (see Decisions made). DES-CODE-MARKERS defines the comment start at each level and where an inline marker belongs. VER-CODE-MARKERS and VER-GRAMMAR-MARKERS cover inline markers and string literals at both levels, and VER-CODE-EDITS renames inline markers.
4. VER-CODE-FACTS expected `project validate` to read no code for a program without code links. The no-read expectation now covers only `query execute`; validation still scans code and reports marker diagnostics.
5. VER-CODE-WORK-COUNTERS counted `query view`, `query gate` and `query report` as reading no code. The reading row now holds every `query` operation of a program that uses code links, the `engineering` readiness gates among them, and the non-reading row only the `query` operations of programs that do not.

Also: ADR-GRAMMAR-TRUST-ON-FIRST-USE (Draft) records the provisional trust policy beside DES-GRAMMAR-ASSETS, and DES-GRAMMAR-ASSETS says how a grammar runs.

Checks after the fix, on a clean tree: `cargo fmt --all --check`, `project validate` (valid: 0 errors, 0 warnings) and `zban check`, each exit 0.
