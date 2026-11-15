---
title: One-shot converter from :::mara
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T15:55:50Z
---

Context: docs/migration.mara.md (REQ-CONVERT-DIRECTIVE-CORPUS).

- [x] Decide the open questions on docs/migration.mara.md (DES-CONVERSION-MAPPING)
- [x] Converter rewrites documents, schema and configuration as DES-CONVERSION-MAPPING describes, and reports what it leaves out
- [x] VER-CONVERSION passes (docs/verification-hosts.mara.md), on the 0.3.0 corpus (`git show v0.3.0:docs/`) and the fixtures it lists, its interruption cases included: the report file `.mara/conversion-report.json` first, then the documents, the configuration last. A stopped conversion leaves a 0.3.0 project refused with `legacy-project` whose report file holds every omission of the documents already converted, and a rerun completes the conversion and returns the whole report. A rerun keeps every recorded entry and adds only entries the file does not hold, so a converted document edited by hand before the rerun keeps its omission and an unchanged unconverted one gets no duplicate (DES-CONVERSION-MAPPING, ADR-CONVERSION-RERUN, ADR-CONVERSION-REPORT-FILE, decided by the maintainer); for `project convert`, so do the cases of docs/verification-writes.mara.md, VER-INTERRUPTED-EDITS among them
- [x] Work-counter test through the real CLI on a small hand-written 0.3.0 fixture, not generator output (the `project convert` row of VER-WORK-COUNTERS, DES-PERFORMANCE-HARNESS); no scaling test (ADR-NO-CONVERTER-SCALING, decided by the maintainer)
- [x] An include pattern of the 0.3.0 configuration that uses brace alternatives or a character class is kept as written and reported, since 0.4.0 patterns have neither and `{`, `}`, `[` and `]` match themselves (DES-CONVERSION-MAPPING, DES-PROJECT-CONFIGURATION; decided by the maintainer in the review of mara-8d6cnw). A case of VER-CONVERSION

## Summary of Changes

- `project convert [--status <status>]` (`crates/mara/src/cli/convert.rs`, `crates/mara/src/convert.rs` and `crates/mara/src/convert/`). It takes the write lock, reads the 0.3.0 configuration, schema and documents, computes every file, and writes them through `write::apply`: the report file, the changed documents in path order, `.mara/schema.toml`, `.mara/config.toml` last. A stopped run leaves a 0.3.0 project that a rerun completes.
  - `config.rs`: `.mara/project.toml` (format 1 to 3) and the converted configuration, whose include patterns are kept as written.
  - `yaml.rs`, `schema.rs`: the YAML schema as a tree with lines, and the converted vocabulary and schema.
  - `discover.rs`: the documents as 0.3.0 found them.
  - `scan.rs`: `:::mara` blocks, metadata, bodies, tokens and Markdown contexts.
  - `documents.rs`: items written with the item writer (`item`, `row`, `link`, `label`, `cell`, `insert`, `add_row`), rows on source items written once, mention links, `<hr>`, ATX headings, and the read-back check.
  - `report.rs`: entries, the report file, and the merge on a rerun.
- `project::find` keeps a 0.3.0 root for the conversion; `select` builds on it. `inputs::compile_gitignore` and `inputs::ignored` are `pub(crate)` for discovery.
- Dependencies of the `mara` crate: `granit-parser` (new, with `arraydeque` and `smallvec`), and `globset`, `pulldown-cmark`, `serde_json` and `toml`, which the lock already held.
- Tests:
  - `crates/mara/tests/convert/` covers VER-CONVERSION. On the corpus read with `git archive v0.3.0`: every MID, ID, field value and relation, a valid project, a complete report, and stops at the report file and after each document, each followed by a rerun that gives the files of an uninterrupted conversion. On a fixture for each mapping row: brace and class patterns, `--status`, label collisions, the two rerun report cases and the report-file refusals.
  - The same directory covers VER-PRESERVED-BYTES, VER-REFUSED-EDITS (both pause points) and VER-INTERRUPTED-EDITS for `project convert`: the four stops at every file of a three-directory fixture, each followed by a rerun.
  - `crates/mara/tests/performance/convert.rs` holds the counter row on a hand-written fixture: 7 reads and hashes (the configuration, the schema, three documents and the two `.gitignore` files of the project), one more for a report file and one more for a `.mara/schema.toml` that an earlier run left, 5 parses, no snapshot build. A `.gitignore` in an ignored directory is not read. No scaling case.
- Docs: DES-CONVERSION-MAPPING (mapping rows and bullets for the decisions below, the result's form), and the failure code `not-a-legacy-project` in DES-OPERATIONS.

Checks, all exit 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`
- `cargo test --workspace --locked` (the `convert` binary: 14 tests, 17 after the review)
- `cargo run --package mara --locked --quiet -- project validate` (valid, 0 errors, 0 warnings)
- `cargo test --package mara --test performance --release --locked` (11 passed, 2 ignored)
- `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`
- `zban check`

By hand, the v0.3.0 corpus converts to a project that `project validate` reports valid, with 32 report entries. So does a project made from the 0.3.0 `minimal` template, with `--status draft`.

## Decisions made

- Selection: `project convert` takes the nearest root with `.mara/config.toml` or `.mara/project.toml`, or `--project`. A root with `config.toml` fails with the new code `not-a-legacy-project` (DES-OPERATIONS). Rejected: reusing `not-a-project`, whose meaning differs.
- Statuses: `--status` takes a status name or label. It joins the status list when no flavour has it, which is what a project made from the 0.3.0 `minimal` template (no status field) needs. Items without a status, or flavours with no status value, refuse the run with `invalid-argument` (exit 2, "a missing option") and a diagnostic at each such item. Rejected: writing items without a status, which would make a valid project invalid.
- Inverse names: a relation without an inverse name, as in every relation of the `minimal` template, gets `inverse_of_<name>`, reported, since a 0.4.0 relation requires one (DES-SCHEMA-MODEL).
- Other relation mappings:
  - `same_flavour` lets each source type that is also a target allow only itself.
  - `code_source` becomes the source `code`.
  - `acyclic` of severity `error` becomes `acyclic = true`. Severity `warning` and `cardinality` are left out and reported, so a valid project stays valid.
  - A relation that allows no source and target type is left out and reported.
- Headings: an item's heading is one level below the last narrative heading before it, at most 6, or level 2 when none precedes it. Rejected: a fixed level, which flattens pages with `##` sections.
- Missing or broken data:
  - A block without a valid `:mid:` gets a new MID, and one without a `:title:` gets its ID as its title. Both are reported. The entry of a new MID names it, and a rerun gives the block the recorded MID (see `## Review`, finding 1).
  - A mention that names no item stays as text, reported.
  - A forward relation row whose target names no item keeps the bare value, which validation reports. An inverse one is left out and reported.
  - An empty field value is left out and reported.
  - Unknown metadata keys become rows, which validation reports, so no value is lost.
- Rows: `Type`, `ID`, the metadata rows in order (a missing status right after `ID`), then the rows from inline relations and other items' inverse assertions, in path and line order.
- Bodies: a thematic break or setext heading that starts a line becomes `<hr>` or an ATX heading. A `<hr>` that text follows directly gets a blank line after it, so that it does not open an HTML block that takes the text in. Narrative keeps its bytes but for its mentions; a setext heading there stays, and validation reports it (one sentence in DES-CONVERSION-MAPPING).
- Discovery as 0.3.0 did it:
  - `globset` with `literal_separator`, so brace and class patterns still find the documents they named; the patterns stay as written in the new configuration and are reported.
  - `.gitignore` files below the root apply.
  - `.git`, symbolic links and other projects' roots are skipped, as in the 0.4.0 inventory.
- YAML is read with `granit-parser`, the parser 0.3.0 used through `serde-saphyr`. Rejected: a hand-written YAML subset.
- The report file is read with `serde_json`, already in the lock.
- Read-back: converted documents do not go through `ReadBack`, whose comparison reads the old `:::mara` blocks as narrative. Instead each new document is parsed once before anything is written, and the run is refused with `unreadable-edit` unless it reads exactly the items written, with their rows. The check compares each item's MID, its rows' keys and values, whether its body holds anything, and that no other item appears. It does not compare narrative or references as the writer's read-back does: it serves "no edit can silently lose or swallow an item". The review noted that it is narrower than `ReadBack`, and the orchestrator kept it. DES-CONVERSION-MAPPING says what it covers. `ReadBack` is unchanged.
- Rerun:
  - A document that holds items and no `:::mara` block is already converted. A rerun changes it only by the rows that other documents' assertions give its items, and computes no entry from it, since its lines are no longer the 0.3.0 lines the report names. Without this, kept tokens were re-reported at new lines and the rerun's report differed from an uninterrupted one.
  - Unchanged documents are not written.
  - The report is the union of the recorded and the computed entries, in path, line and message order, one entry per line.
- Work counts: the conversion parses each document once up front, to find items a stopped run already wrote, plus once per document it writes. It reads `.mara/schema.toml` only when an earlier run left it.
- The converted schema is always `.mara/schema.toml`, as `project init` writes it. The 0.3.0 configuration, schema and rule files stay where they are.
- Values that become no schema name refuse the run like a label collision, with `invalid-project` and diagnostics at the schema lines.
- Decided by the maintainer (after the branch went to review), VER-WORK-COUNTERS: the `project convert` row now also allows the project's `.gitignore` files, and a `.mara/schema.toml` that an earlier run left, beside the leftover report file. As part of the same decision, DES-INPUTS-AND-KEYS in `docs/engine.mara.md` now reads "except the report file and a converted schema that an earlier run left, which only the conversion of a 0.3.0 project reads", so that it agrees with the row. `convert_counts` asserts both reads, with a nested `.gitignore`, a `.gitignore` in an ignored directory that is not read, and a run that finds a report file and a schema left. Question: "The converter (mara-wjy3sh) is done and in review. Its work-counter row in VER-WORK-COUNTERS (Accepted) lets `project convert` read only the files the 0.3.0 configuration declares, plus a leftover report file. The converter also reads `.gitignore` files, because 0.3.0 found its documents that way, and on a rerun the `.mara/schema.toml` an interrupted run left, which the write procedure must read before overwriting it. Changing an Accepted row is yours to decide. Amend the row to allow both?" Chosen option: "Amend the row (Recommended) — The row also allows the `.gitignore` files of the project and a schema file left by an earlier run. The converter then finds exactly the documents 0.3.0 used, and a rerun can safely replace a half-written schema. Nothing else changes; the test already counts these reads." Not chosen: keeping the row and making the converter stop reading `.gitignore` files.

## Review

One review on `a4efd05`; the orchestrator triaged its findings under the maintainer's rules. Fixed in one commit on top of `d2e5bc7`. Each fix has a test through the real CLI that fails without it; I checked this by undoing each fix in turn and running the tests.

1. Generated MIDs survive a stop (`crates/mara/src/convert/documents.rs`, `Conversion::new`, `given_mid`). The report entry of a generated MID now names it: "item `X` has no MID: it gets the MID `…`, which a rerun gives it again". A rerun reads the recorded entries and gives each block without a valid MID, found by its document and ID, the MID that a recorded entry names. Only blocks with no recorded MID get a new one. Test: `report::a_rerun_gives_an_item_the_mid_that_the_stopped_run_gave_it`. It stops after a document that links to a generated MID, reruns, and checks that validation is clean and that the files are those of an uninterrupted conversion apart from the generated MID. DES-CONVERSION-MAPPING records the mechanism.
2. Declared fields come first (`documents.rs`, `Conversion::block`). A metadata key that the item's flavour declares as a field is a field, even when another flavour's relation has that name, as in 0.3.0. Test: `mapping::reads_a_declared_field_as_a_field_before_a_relation_of_its_name`.
3. YAML booleans (`crates/mara/src/convert/yaml.rs`, `Node::boolean`; `crates/mara/src/convert/schema.rs`, `Reader`). Flags take the spellings 0.3.0 took through `serde-saphyr` without strict booleans: `true`, `yes`, `y`, `on` and `false`, `no`, `n`, `off`, in any case. Every value of the schema is read through `Reader`, which reports a value it cannot read as what its key needs and then reads it as absent, never silently. That covers flags, texts (`description`, `id_prefix`, `inverse`, `type`), lists (`values`, `source`, `target`) and `acyclic`, which must be a mapping; `acyclic: true` was read as acyclic before. Test: `mapping::reads_the_yaml_booleans_0_3_0_read_and_reports_what_it_cannot_read`.
4. Merge keys, backlog (mara-gsdycs). A YAML merge key `<<` is no longer read as a key or a declaration. Its report entry says that what it would bring in is not applied, and to write it out in the 0.3.0 schema and convert again, or add it to the converted schema. DES-CONVERSION-MAPPING states the limit in one sentence. Applying merges stays in the backlog.
5. A mention inside a link label, exotic: backlog (mara-gsdycs), no code.

Also in this commit: DES-INPUTS-AND-KEYS, as part of the maintainer's decision on the counter row.

Decisions made in this round:
- A generated MID is recovered by document and ID from the recorded entries, not by line, so a block keeps its MID when lines moved between runs. The report then holds the entry at both lines, as for every moved entry. Rejected: a separate list of MIDs in the report file, which would change its format.
- A schema value that cannot be read as what its key needs is left out and reported, as a conversion leaves out other things, rather than refusing the run.

## Rebase onto 052b7f5

Rebased after `project init`, `item create`/`update`/`delete`, `relation add`/`remove`, `item move`, `item rename` and `project fix` landed. Only `cli.rs` and DES-OPERATIONS conflicted. Each overlap keeps one mechanism:
- CLI: `convert::PROJECT_CONVERT` joins the tip's operation list after `fix::PROJECT`, and `mod convert` joins its modules. `lib.rs` merged without a conflict. `project::find` stays: the tip has no selection that keeps a 0.3.0 root (`project init` reads `recognize` itself). The conversion uses neither `Inputs::admits` nor `Inputs::snapshot`. It keeps the two `pub(crate)` gitignore helpers that `admit.rs` uses as private functions.
- Performance harness: the counter case settles its fixture with the tip's `restore::settle` instead of its own sleep. It needs no `Reads::Writes` case or `Written::learn`: it runs on a hand-written 0.3.0 fixture in a new directory each time, and has no timed runs (ADR-NO-CONVERTER-SCALING).
- Docs:
  - DES-OPERATIONS keeps the tip's new codes and adds `not-a-legacy-project`.
  - The write-order table of DES-WRITE-PROCEDURE gets a Conversion row beside the tip's Fix row, linking to DES-CONVERSION-MAPPING and ADR-CONVERSION-RERUN.
  - The docs index needs no change. Every page stays within 200 lines.
- `Cargo.toml` and `Cargo.lock` merged without a conflict; `granit-parser` is the only new crate.
- mara-0z37ab (the last row of VER-FIX-CASES for `project convert`) is done in this branch: `writes::leaves_the_problems_that_fix_repairs_as_they_were`.
