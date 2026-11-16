---
title: Basic-level comment markers
status: done
type: feature
parent: mara-779tms
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T18:43:42Z
---

Goal: `@mara` comment markers in code files link code to items at file-and-line level: Mara reads them, reports their problems, shows them as relations, and keeps them right when an ID is renamed.

Split by the orchestrator on 2026-11-16, so that this part can be built while alpha.3 and alpha.4 are still in work. Two issues take what depends on those slices: code links as input for programs with the template's code cases (see "Blocks"), and code links through the daemon.

Context: docs/code.mara.md (REQ-CODE-MARKER-LINKS, DES-CODE-MARKERS, DES-CODE-READERS); code keys in DES-INPUTS-AND-KEYS (docs/engine.mara.md); the Verification items in docs/verification-code.mara.md and docs/verification-code-use.mara.md (mara-ttvp9z).

- [x] Decide the open questions on docs/code.mara.md (DES-CODE-MARKERS, DES-CODE-READERS)
- [x] The configuration syntax of code languages, in DES-PROJECT-CONFIGURATION and in the generated corpus configuration
- [x] Per-file scan keyed by content; code layer; edges in `relation list`: VER-CODE-MARKERS passes at the basic level, through the CLI and `mara mcp`; the code-link case of VER-ONE-HOP passes (mara-d005q9 is closed with this issue if this covers it)
- [x] `project validate` reports marker problems; `item delete` is refused while a marker names the item (DES-CODE-READERS, REQ-DELETE-ITEM): VER-CODE-MARKERS and the code row of VER-DELETE-ITEM pass
- [x] `item rename` rewrites the markers that name the old ID, before the `ID` row (REQ-RENAME-ID, DES-WRITE-PROCEDURE): VER-CODE-EDITS at the basic level and the code rows of VER-RENAME-ID and VER-INTERRUPTED-EDITS pass
- [x] The cold rows of VER-CODE-WORK-COUNTERS and the code rows of VER-WORK-COUNTERS that need neither programs nor the daemon pass, with their scaling cases
- [x] The agent guide and the name comparison stay right for whatever this adds to the CLI

## Summary of Changes

Comment markers at the basic level, in direct mode and through `mara mcp`.

- Configuration: optional `[code]` with its own `format_version` (1) and `[code.languages.<name>]` holding `include` and `comment_prefixes` (`mara_engine::config`; DES-CODE-MARKERS has the example, a host test loads it). The generated corpus and the hub declare Rust.
- Scan (`mara_engine::code`): markers read from bytes per LF line, keyed by scanner version + language prefixes + file hash; one scan per distinct key. The code layer is keyed by sorted (path, marker key) pairs. `CodeLinks` resolves markers against the snapshot and reports `invalid-marker`, `unresolved-marker` and `disallowed-target`.
- Inventory (`crates/mara/src/inputs.rs`): code files are inputs only for requests that read code (`CodeReads::None | List | Scan`); `.gitignore`, nested projects and the no-links rule apply; a linked code file is skipped silently; problems that may hide code make the layer incomplete.
- Reads: `relation list` lists an item's code links after its incoming links, and the markers of `<path>` or `<path>:<line>` of a code file; `get` of a code location reads only that file. Code node JSON is `{kind: "code", path, line, declaration}`.
- `project validate` reports marker problems; `item delete` is refused while a marker names the item; `item rename` rewrites markers in code files with mention documents before the `ID` row, fails with `io-error` when code is incomplete, and checks each code edit before writing.
- Tests: `crates/mara/tests/code/` (VER-CODE-MARKERS, VER-CODE-EDITS, the code rows of VER-DELETE-ITEM, VER-INTERRUPTED-EDITS and VER-REFUSED-EDITS, through the CLI and `mara mcp`); the code-link case of VER-ONE-HOP; the cold rows of VER-CODE-WORK-COUNTERS and the hub's code counters and scaling (`project validate`, `relation list`, `item rename`).

Not run here, filed where they belong: daemon cases (mara-9fphtg), grammar-level runs of VER-CODE-EDITS and the grammar `get` row (mara-s9gak4), `query` rows and VER-CODE-FACTS (mara-hk2arx), the `code add` row (mara-h0hxa3). mara-d005q9 is closed by this issue.

Checks: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` (490 passed), `project validate` (valid), the release `performance` test (16 passed, 2 ignored), the `wasm32` build of `mara-engine`, `zban check`: all exit 0.

## Decisions made

- Configuration syntax: `[code]` with `format_version` 1 (the marker syntax is a public format) and `[code.languages.<name>]` with `include` patterns instead of extensions, consistent with `[documents]`. Written in DES-CODE-MARKERS and DES-PROJECT-CONFIGURATION.
- A file matched by two languages belongs to the first in the configuration's order; a document or a write's temporary file is never a code file (DES-CODE-MARKERS, DES-WRITE-PROCEDURE).
- `@mara` may follow the comment prefix directly (`//@mara`), as 0.3.0 read it; where several prefixes start at the same place, the longest is the comment start (`///`). Limit stated: a prefix that is also an operator, such as `*`, starts a comment after a space, so `a * b; // @mara …` holds no marker (DES-CODE-MARKERS).
- A `disallowed-target` marker still links (as VER-CODE-FACTS expects); an unresolved or invalid one does not.
- `item rename` rewrites every marker that names the old ID, including one whose relation is undeclared; the edit replaces only the ID bytes and is checked instead of read back (`unreadable-edit` on a failed check) (DES-CODE-READERS).
- Code is incomplete also when a directory or `.gitignore` that may hide a code file cannot be read or applied; `item rename` then fails with `io-error` (DES-CODE-READERS).
- An item's code links follow its incoming links in path and line order; the code node is `{kind: "code", path, line, declaration}`, `line` null for a whole file (DES-READ-RESULTS).
- `get` of a code location lists code files without reading them and reads only that one (DES-CODE-READERS).
- `item validate`, `project fix`, `item move` and every other operation read no code, per DES-CODE-READERS' table.

## Rejected approaches

- Keeping every code file's contents in the layer: only files with markers keep them, which is all a rename needs.
- A separate scan of the new contents after a rename: the counters forbid it, and the byte check covers what it would.

## Review fixes

The review of `703569f` found no defect in what `item rename` writes into source files. Fixed:

1. `item delete` wrote while code was incomplete. It now refuses with `io-error` naming each unreadable file, directory or `.gitignore`, before writing, as `item rename` does; both use one check (`cli/author.rs`, `incomplete_code`). No other write reads code. Test: `edits::refuses_to_delete_while_code_cannot_be_read` (unreadable file, unreadable `src/`, a `src/.gitignore` that is not UTF-8; CLI and `mara mcp`).
2. `item delete` counted only resolved links. `CodeLinks` now indexes every marker by the ID it names, whatever its relation; delete refuses on each marker naming one of the item's IDs. Test: `edits::refuses_to_delete_an_item_that_any_marker_names` (an undeclared relation; a disallowed target).
3. `relation list` of a code file made the target's summary once per marker. Summaries are now made once per item per request, for document links too. Scaling case: `relation list src/markers.rs` on the hub with one marker per source naming the hub (`scaling::growth_per_doubling`); measured 1.96× per doubling with the fix, 2.72× without.
4. `[code]` without `languages` was an error. It now declares no language. Tests: `config::tests::reads_code_languages_…` and `markers::a_code_table_without_languages_declares_none`.
5. `get` refused a code file whose name ends in `.mara.md`; the inventory now decides. Test: `markers::a_code_file_may_end_in_mara_md`.

Each new test fails with its fix reverted. DES-CODE-READERS states the delete rules.
