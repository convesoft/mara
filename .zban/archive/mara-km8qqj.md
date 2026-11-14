---
title: Write the Verification items for the alpha.1 requirements
status: done
type: task
priority: high
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-14T13:27:36Z
---

Goal: every requirement that the alpha.1 slice delivers has an accepted Verification item: the requirements that the issues of mara-t1rffa name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [x] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [x] Include REQ-SOURCE-IS-PUBLISHED-FORM and REQ-STABLE-IDENTITY, which no issue names
- [x] One Verification item for each, or one shared by requirements that a single check covers
- [x] The test items in those issues point at the Verification items instead of restating them

## Summary of Changes

The slice delivers 20 requirements. Each now has a Verification item:

| Requirement | Verification |
| --- | --- |
| REQ-SOURCE-IS-PUBLISHED-FORM | VER-GITHUB-RENDERING |
| REQ-STABLE-IDENTITY | VER-MID-RESOLUTION |
| REQ-SAME-RESULT-ON-EVERY-HOST (through DES-ENGINE-BOUNDARY, mara-0ztabc) | VER-HOST-COMPARISON (alpha.1 runs only its architecture checks) |
| REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING | VER-WORK-COUNTERS, VER-SCALING (already there) |
| REQ-ONE-PROJECT-PER-OPERATION | VER-NESTED-PROJECTS |
| REQ-SCHEMA-DEFINES-VOCABULARY | VER-CUSTOM-SCHEMA |
| REQ-SCHEMA-INSPECTION | VER-SCHEMA-INSPECTION |
| REQ-REPORT-EVERY-PROBLEM, REQ-VALIDITY-COVERS-WHOLE-PROJECT | VER-SEEDED-PROBLEMS |
| REQ-INVALID-ROWS-REPORTED | VER-INVALID-ROWS |
| REQ-READS-NEVER-WRITE | VER-UNCHANGED-SOURCES |
| REQ-SEARCH-ITEMS-AND-NARRATIVE, REQ-SEARCH-RANKING, REQ-TYPO-TOLERANT-SEARCH | VER-SEARCH-CASES |
| REQ-READ-NODE | VER-NODE-READS |
| REQ-DIRECT-NEIGHBOURS | VER-ONE-HOP |
| REQ-LIST-ITEMS | VER-ITEM-LIST |
| REQ-BOUNDED-PAGES | VER-CURSOR-WALK |
| REQ-PAGE-LIMITS (Draft) | VER-PAGE-LIMITS (Draft) |

- New pages docs/verification-documents.mara.md and docs/verification-retrieval.mara.md, listed in docs/index.mara.md; VER-HOST-COMPARISON added to docs/verification.mara.md.
- Test items now name the Verification items: mara-0ztabc, mara-000f9q and mara-bmc9gc (checklists); mara-t6xn9z and mara-b52dg3 (context). mara-csmk0c and mara-8d6cnw already named theirs. No status changed.
- Filed: mara-5fcaqw (VER-WORK-COUNTERS warm rows before the daemon; needs the maintainer), mara-s2b1gg (nested projects), mara-d005q9 (code-link case of VER-ONE-HOP, alpha.5).

Checks: `zban check` reports nothing. A hand-check script over `docs/` found no problems: 29 pages, 168 items, 287 links. It checked MID form and uniqueness, `---` with blank lines on both sides before every anchor, the anchor/heading/table order, unique IDs, every link's path, MID and link text, the index listing every page, and every page at or under 200 lines (110, 150, 156). Seeded defects in a copy were all caught. A coverage script confirmed that every accepted requirement above has an accepted Verification and that no accepted Verification verifies a draft requirement. Every new MID came from the AGENTS.md command.

## Decisions made

- VER-PAGE-LIMITS is Draft because REQ-PAGE-LIMITS is Draft: the strict `Verifies` relation forbids an accepted Verification of a draft requirement (DES-TEMPLATE-RELATION-LIFECYCLE). VER-TIME-BUDGETS set the precedent. Both become Accepted together.
- REQ-SAME-RESULT-ON-EVERY-HOST is listed because mara-0ztabc names DES-ENGINE-BOUNDARY. In alpha.1 only the architecture checks of VER-HOST-COMPARISON run: the engine crate does no I/O and builds for `wasm32`. They do not establish the requirement. Its parity cases wait for the daemon (alpha.4, mara-wz40n6) and for the Node library and browser hosts (alpha.6, mara-vfkgbs).
- Analysis against EVD-GITHUB-FRAGMENT-MATCHING establishes GitHub navigation, not a manual github.com check per release. Rejected: a per-release Inspection that no worker can run before a push.
- Each case states its pass criteria through the CLI, and a case that needs operations from both mara-000f9q and mara-bmc9gc is assigned to each issue separately, because neither issue blocks the other.
- Pass criteria leave open what the docs leave open: the kind of edit distance, the fields of an `item list` summary, the response to a page limit outside 1–100, and whether `--project` accepts a subdirectory. The implementing issues decide these.
