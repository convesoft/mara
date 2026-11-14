---
title: Validate projects and items
status: done
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-8d6cnw
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T18:24:43Z
---

Goal: `project validate`, `item validate` and `schema validate` report every built-in check failure with its location.

Context: docs/validation.mara.md (REQ-REPORT-EVERY-PROBLEM, REQ-VALIDITY-COVERS-WHOLE-PROJECT, REQ-READS-NEVER-WRITE, REQ-INVALID-ROWS-REPORTED, DES-SCHEMA-CHECKS, DES-DIAGNOSTICS).

- [x] Decide diagnostic codes and the result format (DES-DIAGNOSTICS)
- [x] Built-in checks, linear in items plus edges. The Document structure check and malformed rows are the parser's own diagnostics, `mara_engine::document::Document::diagnostics` (DES-SCHEMA-CHECKS, mara-t6xn9z)
- [x] CLI operations with human and JSON output
- [x] VER-SCALING covers validation, which times the cold parse of every document: the parser has no case of its own (DES-PERFORMANCE-HARNESS, mara-t6xn9z)
- [x] The cold rows of VER-WORK-COUNTERS (docs/verification.mara.md) pass through the real CLI for the validation operations: a cold request (two documents with the same contents included: 1 parse per distinct content), `.git` and undeclared files, an operation that reads no code (moved here from mara-8d6cnw by the maintainer, mara-63dn2d)
- [x] VER-SEEDED-PROBLEMS, VER-CUSTOM-SCHEMA and VER-GITHUB-RENDERING pass (docs/verification-documents.mara.md)
- [x] The `project validate` case of VER-INVALID-ROWS passes, and VER-NESTED-PROJECTS and VER-UNCHANGED-SOURCES pass for the validation operations
- [x] The validation operations select and load the project with `mara::project::select` and `load` (crates/mara/src/project.rs, mara-b52dg3), which only host tests have exercised. Real-CLI tests cover, with the codes of DES-PROJECT-CONFIGURATION: selection from a subdirectory, from inside a nested project and with `--project` (absolute, relative, naming a non-root: `not-a-project`); `no-project` outside every project; `legacy-project` naming `mara project convert`; an unknown configuration setting (`unknown-key`) reported while the schema and documents are still checked; a schema file that does not exist (`io-error` at the configuration line naming it); an invalid schema declaration left out without hiding the other problems; a `format_version` other than 1 (`unsupported-format-version`); a schema path into a nested or 0.3.0 project (`outside-project` at the configuration line, the nested schema unread); a symbolic link at `.mara`, at `.mara/config.toml` or at a component of the schema path (`symbolic-link`, nothing behind it read); configuration and schema errors never refuse validation, which reports them with the rest (mara-zv9d69)

## Summary of Changes

- Engine (`crates/mara-engine`):
  - `diagnostic.rs`: `Diagnostic` gains `id` and `mid` (DES-DIAGNOSTICS), `Diagnostic::error` and `write_json`; `Severity::as_str`.
  - `json.rs` (new): the JSON writer of results, so every host prints the same JSON and a page's size is the sum of its records' sizes.
  - `paging.rs` (new): REQ-PAGE-LIMITS numbers, `parse_limit`, `Cursor` (result key + offset) and `fill` (record limit and 65,536-byte budget). Shared with the read operations of mara-bmc9gc.
  - `validation.rs`, `validation/checks.rs` (new): `check` runs the built-in checks (one pass over items, one over edges, Tarjan over each acyclic relation's edges; each visit one check step) and attaches each parse problem to the item that holds its line; `Finding` marks the copy of a shared problem that only item validation shows; `Report`, `Page`, `result_key`.
  - Parser: only `Item::end_line` added (`document.rs`, `document/region.rs`), to find the item of a parse problem. `snapshot.rs` unchanged. `hash.rs`: `Hash::from_hex` for cursors.
- Host (`crates/mara`):
  - `src/cli.rs` (new): the CLI skeleton. `OPERATIONS` table; an `Operation` is its words, positional arguments, options (each with a value check run before anything) and `fn(&mut Context, &Args) -> Result<Response, Failure>`. Global options `--project`, `--format human|json`, `--debug-stats`; shared `LIMIT` and `CURSOR` options; usage on bare `mara`; exit statuses; failure output in both forms.
  - `src/cli/validate.rs` (new): `project validate [--path]... [--limit] [--cursor]`, `item validate <id-or-mid>`, `schema validate`. Validation is never refused.
  - `src/main.rs` thin; `project.rs` boxes its private `Diagnostic` errors (clippy, the struct grew); `inputs.rs` uses the constructor.
- Tests:
  - `crates/mara/tests/validation/` (new, 24 real-CLI tests in disposable projects under `$TMPDIR`): VER-SEEDED-PROBLEMS (29 seeded problems, exact list; schema/item validate; paging and `--path`; fixed project valid), VER-INVALID-ROWS (`project validate`), inventory problems, VER-CUSTOM-SCHEMA, VER-GITHUB-RENDERING (with the Analysis and Inspection written in the test's doc comment), VER-NESTED-PROJECTS (all seven cases, file-read counts per operation), VER-UNCHANGED-SOURCES (names, bytes and mtimes after 11 runs, failing ones included), and the selection/loading list (subdirectory, `--project` absolute/relative/non-root, `no-project`, `legacy-project`, `unknown-key`, missing schema, `unsupported-format-version`, `outside-project` into a nested and a 0.3.0 project with only the configuration read, `symbolic-link` at `.mara`, `.mara/config.toml` and a schema path component).
  - Performance harness: `cli::cases` holds one case per operation, run by both `cold_counts_at` and `growth_per_doubling`; the cold request adds two documents with the same contents (1 parse for both) and asserts exact reads, hashes, parses, snapshot builds, 0 code scans, 0 subprocesses, 0 program runs, and items + edges ≤ check steps ≤ 2 × (items + edges). The corpus exposes `edges` and `first_id`.
- Docs: DES-SCHEMA-CHECKS gets the codes of the built-in checks and their rules; DES-DIAGNOSTICS gets targets, paging, `--path`, the result key and the JSON and human forms; DES-OPERATIONS gets the CLI's global options, exit statuses, JSON failure form and failure codes.

Checks: all six local checks of AGENTS.md pass; the nightly 64× counter run passes. Growth per doubling at 1×→4×→16× (release, medians): `project validate` 1.92 and 2.07 (4.4, 16.2, 69.8 ms), `item validate` 1.80 and 1.89, `schema validate` flat (0.4 ms); the cold snapshot build alone is 2.10.

This repository's `docs/`, copied with the configuration example of DES-PROJECT-CONFIGURATION and the engineering schema of the corpus (`crates/mara/tests/performance/schema.toml`, DES-ENGINEERING-VOCABULARY), validates with no diagnostic (29 documents); corrupting a link path and a link text in the copy gives `stale-link-path` and `stale-link-text`.

Rejected approaches:

- `clap`: the hand-written table gives the rejection diagnostic, the JSON failure and the stats line exactly as specified, with no dependency.
- `serde_json` for output: per-record page sizes must be computed once, and every host must print the same JSON; the engine writes it.
- Reporting a repeated MID or ID at every holder: one seeded problem would appear twice in `project validate`.
- Validating every item an ambiguous handle names: refused with `ambiguous-item`, naming the places.

## Decisions made

- Exit status 0 for a valid result or a successful operation, 1 for an invalid result or a failed request, 2 for rejected arguments, as 0.3.0 (DES-OPERATIONS).
- `--format json` prints one object on stdout, also for a failure: `{"format_version":1,"error":{"code","message","diagnostics"}}`; the human form prints a result on stdout and a failure on stderr as `mara: error[<code>]: <message>` (DES-OPERATIONS). Failure codes `invalid-argument`, `unknown-item`, `ambiguous-item`, `stale-cursor`, `record-too-large`.
- Global options stand anywhere; an operation's options follow its words. Bare `mara` prints the usage and still exits 0 with no work, so the harness case is unchanged (DES-OPERATIONS).
- Built-in check codes and where each is reported (DES-SCHEMA-CHECKS): `undeclared-type`, `missing-row`, `duplicate-row`, `repeated-key`, `undeclared-key`, `invalid-value`, `invalid-id`, `wrong-id-prefix`, `duplicate-mid`, `duplicate-id`, `bare-id-value`, `invalid-relation-value`, `broken-link`, `stale-link-path`, `stale-link-text`, `disallowed-target`, `relation-cycle`. A same-type violation is a `disallowed-target`, since loading keeps only the source's own type as target.
- A problem shared by several items (repeated MID or ID, cycle) is reported once to the project, at the later holder or the first link, and to each item's validation at that item (DES-SCHEMA-CHECKS).
- A check that needs a declaration the schema lacks does not run, without a schema or for an undeclared type; link paths are percent-decoded and resolved like relative URLs, absolute ones are stale; messages quote at most 100 characters of a value and list at most 10 names followed by `and <N> more` for the rest, all through one function, `mara_engine::validation::list`, failure messages (`ambiguous-item`, an incomplete operation) included (DES-SCHEMA-CHECKS, DES-OPERATIONS).
- The first `Type` row gives the item's type. A row's key is a field of that type, a relation that the type is a source of, or neither (`undeclared-key`). Only a relation row is an edge: a row keyed by a relation the type is not a source of is `undeclared-key` and no edge, so `relation list` does not show it and its link is not checked (DES-SNAPSHOT, DES-SCHEMA-CHECKS; `Schema::relation_from`).
- Every row's value is checked on its own, repeated and duplicated rows included; the first row of a single-valued key gives the item's value; `undeclared-type` is reported at each `Type` row that names no type; a row that repeats key and value is `duplicate-row` instead of `repeated-key`; a shared problem stays once per item when rows repeat: an exact repeat of an `ID` row has its own form and prefix checked, not the uniqueness of its value (DES-SCHEMA-CHECKS).
- The snapshot's ID map holds each item once under each of its `ID` values, so `items_with_id` lists distinct items (DES-SNAPSHOT).
- Validation pages with `--limit`, `--cursor` and the byte budget like reads, as 0.3.0; `project validate --path` is repeatable and relative to the project root, and as in 0.3.0 drops empty and `.` parts and a trailing `/` and rejects an absolute path, a `..` part or the root itself; the normalized paths, sorted and without repeats, enter the result key (DES-DIAGNOSTICS).
- A selection problem with a directory's `.mara` (a symbolic link, or one that cannot be examined) keeps its location in the failure: `.mara/config.toml` line 1 of that directory, or the file in `.mara` that cannot be examined; `no-project`, `not-a-project` and `legacy-project` lie in no project and have none (DES-PROJECT-CONFIGURATION).
- `schema validate` reports the schema file's diagnostics and the configuration's at the line that names the schema, or every configuration diagnostic when it names none, and reads no document; `item validate` takes an ID or a MID (DES-DIAGNOSTICS).
- The JSON result is `format_version`, `valid`, `errors`, `warnings`, `diagnostics`, `next_cursor`; a diagnostic is `code`, `severity`, `message`, `path`, `line`, `id`, `mid` (DES-DIAGNOSTICS).
- A validation result's key covers the code, path and line of the diagnostics no input key covers (DES-DIAGNOSTICS).
- A check step is one visit by one pass: each item twice (the pass that collects its first `Type` and `ID`, then the item pass), each edge once, and the items and edges of each acyclic relation once more; the counter definition of DES-PERFORMANCE-HARNESS is unchanged. Work per row, per holder of a shared key and per finding is gated by scaling cases instead, one project per shape: repeated `ID`, `Type`, `Status` rows, other values of a single-valued enum field, relation rows to one target or to many, undeclared keys (1000 × size rows of one item), one ID or MID shared by 500 × size items, 1000 × size broken links on one line, and the hub (DES-PERFORMANCE-HARNESS).

## Review fixes (second commit)

An independent review found five defects; all are fixed on top of 0e21515.

1. A row is read as a field or a relation by the item's type: `Schema::relation_from` decides in the snapshot's edge construction and in the checks. A text field that shares a relation's name is no longer an edge or `invalid-relation-value`.
2. The checks collect each item's first `Type` and `ID` once and look schema lists up in sets, so no edge or row rescans an item's rows or a list; a cycle's members are grouped in one pass. A hub case (late metadata, many incoming edges) joins the counter test (exact steps) and the scaling test; with the old per-edge scan it grows 2.54× per doubling and fails.
3. Every row's value is checked, a repeated `Type` included.
4. A symbolic or unreadable `.mara` met during selection keeps its location `.mara/config.toml:1` in the failure (`SelectError::diagnostic`).
5. `--path` normalizes `.` parts, repeated and trailing `/` as 0.3.0 did, before filtering and before the result key.

Checks: all six local checks pass on the clean final commit; the 64× counter run passes.

## Review fixes (third commit)

1. Repeated `ID` rows no longer make validation quadratic: the snapshot's ID map holds each item once per value, so finding another holder takes constant time, and uniqueness is checked once per distinct ID on each item. The old path fails the new `RepeatedId` scaling case at 2.97× per doubling. Every loop of `checks.rs` that runs per row, holder, finding or message was audited: each is bounded by a hash lookup, by the item's own rows or by the 10-name cap.
2. An exact repeat of an `ID` row no longer reports the cross-item conflict again; each holder's `item validate` shows it once.
3. The 10-name cap and its `and <N> more` wording are in DES-SCHEMA-CHECKS.

Ten scaling cases, one project per shape, gate work that check steps do not count; they run in their own test, `per_row_growth_per_doubling`, beside `growth_per_doubling`. Checks: all six local checks pass on the clean final commit; the 64× counter run passes.

## Review fix (fourth commit)

The cycle message omitted the `and` of `and <N> more`. Every message that lists names (statuses, enum values, allowed target types, cycle members, the holders of an ambiguous handle, the operations an incomplete operation starts) now lists them through `mara_engine::validation::list`. A unit test covers the function; a real-binary test covers each validation listing and `ambiguous-item` with more than ten names. Checks: all six local checks pass on the clean final commit.
