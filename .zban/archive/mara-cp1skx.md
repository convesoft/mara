---
title: Link this repository's code to the requirements it implements
status: done
type: task
parent: mara-779tms
blocked_by:
  - mara-9fphtg
discovered_from: mara-779tms
created_at: 2026-11-21T19:15:35Z
closed_at: 2026-11-21T20:30:15Z
---


Goal: the outcome of the 0.4.0-alpha.5 epic holds for this repository itself: its configuration declares Rust as a code language, its code carries `@mara` markers on what implements each requirement, and Mara's own reads show those links.

Context: the epic mara-779tms is "Done when this repository's code links to the requirements it implements". At `d6da3c9` no issue did that: `.mara/config.toml` has no `[code]` table, and the only `@mara` text under `crates/` is in tests, fixtures and messages. Filed by the orchestrator on 2026-11-21. Items: REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS and DES-CODE-READERS (docs/code/index.mara.md); the query `implemented` and the gate `verified` of the engineering template (`.mara/config.toml`, `.mara/rules/readiness.dl`); the relation the vocabulary gives a code link (docs/template/vocabulary.mara.md).

Decided by the orchestrator, provisional (the maintainer may overturn it): the repository declares Rust at the basic level, without a pinned grammar. With a grammar pinned, `project validate`, one of the local checks, fails with `unavailable-grammar` on every machine that has not run `mara code add rust`, which downloads; the local checks stay free of downloads until the maintainer says otherwise. Moving to the declaration level later is `mara code add rust` and no change to the markers.

- [x] `.mara/config.toml` declares `[code]` with Rust at the basic level; its include patterns cover the crates' source and leave out files whose `@mara` text is test data (fixtures, test strings), so that no marker resolves to nothing
- [x] Every Accepted requirement that the code implements today has at least one `@mara implements <ID>` marker, at the function, type or module that implements it; no marker claims a requirement that is not built yet. Requirements only: markers for designs and verifications are a follow-up issue if they prove useful
- [x] `project validate` reports no error and no new warning; `query execute implemented` lists the links; the closing summary lists each Accepted requirement that has no marker, with the slice or issue that will build it
- [x] AGENTS.md says in one line, under "Mara documents", that code implementing a requirement carries its marker and that markers change with the code; README and other pages only link

Done when `cargo run --package mara --locked --quiet -- --no-daemon query execute implemented --limit 100` in this repository lists a code link for each requirement the summary names as implemented, and the seven local checks pass.

## Summary of Changes

- `.mara/config.toml`: `[code]` with `format_version = 1` and `[code.languages.rust]` at the basic level, with prefixes `//` and `///` and no `grammar`. Include patterns: `crates/mara/src/**/*.rs`, `crates/mara-engine/src/*.rs`, `crates/mara-engine/src/code/outline.rs` and one `crates/mara-engine/src/<dir>/**/*.rs` for each other engine subdirectory. That is 109 files: `--debug-stats` reports `code_scans: 109`. A TOML comment says why the subdirectories are listed. With `crates/**/*.rs`, the scan read 36 markers, all in `crates/*/tests/` and `crates/mara-engine/src/code/tests.rs`: 27 `invalid-marker` and 9 `unresolved-marker`, none resolving to a real item. No other source file holds a marker, and no comment had to change.
- 94 markers `// @mara implements <ID>` on 76 declarations, linking 43 requirements. They follow the convention in AGENTS.md ("Mara documents"): a marker on each function or type that carries out a distinct obligation, for the hosts that exist. The first version (975ec2b) had 43 markers on 41 requirements, and its review found 24 requirements that pointed at a data type, a helper or only part of the implementation; this version replaces it.
- AGENTS.md (98 lines): one line under "Mara documents" with this repository's marker convention, linking to DES-CODE-MARKERS.

### Requirements and marked declarations
- REQ-AGENT-GUIDE: `render` (mara/src/cli/guide.rs)
- REQ-BOUNDED-PAGES: `fill_within` (mara-engine/src/paging.rs), `page_of` (mara-engine/src/read/node.rs), `list` (mara-engine/src/read/page.rs), `page` (mara-engine/src/validation.rs)
- REQ-CLI-MCP-PARITY: `OPERATIONS` (mara/src/cli.rs), `parse` (mara/src/cli.rs), `call_tool` (mara/src/cli/mcp.rs), `tool` (mara/src/cli/mcp.rs), `args` (mara/src/cli/mcp.rs)
- REQ-CODE-MARKER-LINKS: `scan` (mara-engine/src/code.rs), `scan_syntax` (mara-engine/src/code.rs), `new` (mara-engine/src/code.rs), `relation_list` (mara-engine/src/read/relations.rs)
- REQ-CONVERT-DIRECTIVE-CORPUS: `convert` (mara/src/convert.rs), `select` (mara/src/project.rs)
- REQ-CREATE-ITEM: `create` (mara-engine/src/authoring/create.rs), `item_create` (mara/src/cli/author.rs)
- REQ-DELETE-ITEM: `delete` (mara-engine/src/authoring/delete.rs)
- REQ-DIRECT-NEIGHBOURS: `relation_list` (mara-engine/src/read/relations.rs)
- REQ-EDIT-RELATIONS: `relation_add` (mara-engine/src/authoring/relations.rs), `relation_remove` (mara-engine/src/authoring/relations.rs)
- REQ-EXPLAIN-OUTCOMES: `explain` (mara/src/rules/explain.rs)
- REQ-EXTERNAL-RECORDS-AS-SUPPLIED: `check_inputs` (mara-engine/src/facts/inputs.rs), `supplied_input` (mara/src/rules.rs)
- REQ-FIRST-CALL-IN-PROCESS: `request` (mara/src/cli/client.rs)
- REQ-INTERRUPTED-EDIT-LOSES-NOTHING: `move_item` (mara-engine/src/relink.rs), `item_rename` (mara/src/cli/relink.rs), `convert` (mara/src/convert.rs), `apply` (mara/src/write.rs), `item` and `edge` (mara-engine/src/validation/checks.rs: the duplicate-MID and stale-link checks)
- REQ-INVALID-ROWS-REPORTED: `metadata_rows` (mara-engine/src/document/region.rs), `row` (mara-engine/src/validation/checks.rs)
- REQ-LIST-ITEMS: `item_list` (mara-engine/src/read/search.rs)
- REQ-MINIMAL-REWRITE: `edit` (mara-engine/src/write.rs), `rename` (mara-engine/src/code.rs), `replace` (mara/src/write.rs)
- REQ-MOVE-ITEM: `move_item` (mara-engine/src/relink.rs)
- REQ-NEVER-STALE: `generation` (mara/src/inputs.rs), `check` (mara/src/tracked.rs)
- REQ-ONE-FIX-OPERATION: `plan` (mara-engine/src/fix.rs), `project_fix` (mara/src/cli/fix.rs)
- REQ-ONE-PROJECT-PER-OPERATION: `directory` (mara/src/inputs.rs), `select` (mara/src/project.rs)
- REQ-ONLY-CODE-ADD-DOWNLOADS: `code_add` (mara/src/cli/code.rs), `new` (mara/src/inputs.rs)
- REQ-PROGRAM-FAULTS-REPORTED: `judge` (mara-engine/src/evaluation/gate.rs), `interpret` (mara-engine/src/evaluation/souffle.rs), `diagnostic` (mara-engine/src/program.rs)
- REQ-PROGRAMS-ONLY-COMPUTE: `admit` (mara-engine/src/program.rs), `new` (mara/src/rules/sandbox.rs), `apply` (mara/src/rules/sandbox.rs)
- REQ-PROJECT-AUTHORED-LOGIC: `read` (mara-engine/src/config/rules.rs), `admit` (mara-engine/src/program.rs), `evaluate` (mara/src/rules.rs)
- REQ-PROJECT-INIT: `project_init` (mara/src/cli/init.rs)
- REQ-READ-NODE: `get` (mara-engine/src/read/node.rs)
- REQ-READS-DURING-VALIDATION: `accept` (mara/src/cli/daemon/server.rs), `generation` (mara/src/inputs.rs)
- REQ-REFUSED-EDIT-CHANGES-NOTHING: `apply` (mara/src/write.rs)
- REQ-RENAME-ID: `rename` (mara-engine/src/relink.rs), `item_rename` (mara/src/cli/relink.rs)
- REQ-REPORT-EVERY-PROBLEM: `project_validate` (mara/src/cli/validate.rs), `item_validate` (mara/src/cli/validate.rs)
- REQ-SAME-RESULT-ON-EVERY-HOST: `request` (mara/src/cli/client.rs), `answer` (mara/src/cli/client.rs)
- REQ-SCHEMA-DEFINES-VOCABULARY: `parse_schema` (mara-engine/src/schema.rs), `item` (mara-engine/src/validation/checks.rs), `edge` (mara-engine/src/validation/checks.rs)
- REQ-SCHEMA-INSPECTION: `schema_list` (mara-engine/src/read/schema_view.rs), `schema_get` (mara-engine/src/read/schema_view.rs)
- REQ-SEARCH-ITEMS-AND-NARRATIVE: `search` (mara-engine/src/read/search.rs)
- REQ-SEARCH-RANKING: `search` (mara-engine/src/read/search.rs)
- REQ-SOURCE-IS-PUBLISHED-FORM: `parse` (mara-engine/src/document.rs), `link` (mara-engine/src/write.rs), `item` (mara-engine/src/write.rs)
- REQ-STABLE-IDENTITY: `update` (mara-engine/src/authoring/update.rs), `move_item` (mara-engine/src/relink.rs), `rename` (mara-engine/src/relink.rs), `add_edge` (mara-engine/src/snapshot.rs), `targets` (mara-engine/src/read/relations.rs)
- REQ-TYPO-TOLERANT-SEARCH: `nodes` (mara-engine/src/read/search.rs), `edits_allowed` (mara-engine/src/snapshot/spelling.rs)
- REQ-UNAVAILABLE-NOT-PARTIAL: `judge` (mara-engine/src/evaluation/gate.rs), `source_complete` (mara-engine/src/facts.rs), `spawn_and_wait` (mara/src/rules.rs)
- REQ-UPDATE-ITEM: `update` (mara-engine/src/authoring/update.rs)
- REQ-VALIDITY-COVERS-WHOLE-PROJECT: `respond` (mara/src/cli/validate.rs)
- REQ-WORK-FOLLOWS-CHANGE: `generation` (mara/src/inputs.rs), `consume_with` (mara/src/rules/consumer.rs), `evaluate` (mara/src/session.rs)
- REQ-WRITERS-SERIALIZED: `take` (mara/src/write.rs), `apply` (mara/src/write.rs)
### Checks
- `project validate`: before, `valid: 0 errors, 1 warning`. After, the same: the one warning is still `requirement-verified` at `docs/distribution.mara.md:7`, and there is no marker diagnostic. Time (debug build, `--no-daemon`, hyperfine, 3 runs): 118.8 ms ± 1.2 before, 154.8 ms ± 0.8 after.
- `cargo run --package mara --locked --quiet -- --no-daemon query execute implemented --limit 100`: 94 rows and no cursor. That is 43 requirements, each listed above with every marked declaration. All are direct code links; none go through a design.
- All seven local checks pass: fmt, clippy with warnings denied, `cargo test --workspace` (26 test binaries, 0 failed), `project validate`, the release performance test (32 passed, 2 ignored), the `wasm32` build and `zban check`. No core dumps.

### Accepted requirements without a marker (8)
- REQ-NODE-LIBRARY: alpha.6, mara-vfkgbs.
- REQ-BROWSER-LOCAL-WORKSPACE: alpha.6, mara-jptfbq.
- REQ-BROWSER-ACCESS-AUTHORIZED: alpha.6, mara-fkkcad (the daemon's loopback HTTP).
- REQ-RUN-WITHOUT-TOOLCHAIN: 0.4.0, mara-qzp1gd / mara-a4m588.
- REQ-NEAR-LINEAR-SCALING and REQ-READS-NEVER-WRITE: built, but they are properties of every operation, with no one declaration to send a reader to.
- REQ-TEMPLATE-ACCEPTANCE-RULES and REQ-TEMPLATE-GATES-PROVE-READINESS: built only in `.mara/rules/*.dl`, and programs are not code files.

## Decisions made
- The repository's marker convention is the AGENTS.md line under "Mara documents": where markers go, which requirements carry none, and checking the `[code]` patterns when a file or directory is added under `crates/mara-engine/src/`.
- Include patterns list the engine's subdirectories so that `crates/mara-engine/src/code/tests.rs` stays out. Patterns have no exclude, and changing its test data would change tests. The TOML comment in `.mara/config.toml` records this.
- A marker is a plain `//` line directly above the declaration's doc comments and attributes. At the grammar level it then belongs to that declaration, so `mara code add rust` needs no change to the markers.
- REQ-EXTERNAL-RECORDS-AS-SUPPLIED and REQ-UNAVAILABLE-NOT-PARTIAL keep only their Rust parts. Telling a missing record from an unavailable one is in `.mara/rules/readiness.dl`, which cannot be marked.

## Rejected approaches
- Patterns `crates/**/*.rs` and `crates/*/src/**/*.rs`: both include marker-shaped test data, giving 36 and 11 errors.
- Escaping the test data in `code/tests.rs`: it would change tests, and the comments themselves needed no change.
- Mostly one marker per requirement (975ec2b, 43 markers on 41 requirements): it sent readers and `implemented` to half the implementation.
