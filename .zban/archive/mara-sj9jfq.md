---
title: project init with the engineering and empty templates
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-15T15:12:38Z
---

Context: REQ-PROJECT-INIT in docs/project.mara.md; DES-ENGINEERING-VOCABULARY in docs/vocabulary.mara.md.

- [x] Decide which template applies when none is named: `engineering` (REQ-PROJECT-INIT)
- [x] Decide the inverse relation names (DES-ENGINEERING-VOCABULARY)
- [x] Templates embedded in the binary; existing files are never overwritten; created files are reported
- [x] VER-PROJECT-INIT passes (docs/verification-hosts.mara.md), with the `project init` cases of VER-REFUSED-EDITS (docs/verification-writes.mara.md); work-counter and scaling tests (the `project init` row of VER-WORK-COUNTERS, VER-SCALING)

Schema source (from mara-37qtf2): the `engineering` template's schema must be the same text as this repository's `.mara/schema.toml`, which declares DES-ENGINEERING-VOCABULARY (docs/vocabulary.mara.md, descriptions included). Keep the one source inside the crate that embeds the template, for example `crates/mara/templates/engineering/schema.toml` read with `include_str!`, not in `.mara/`: a crate must not embed a file outside its package, and the product's template must not change with this repository's own configuration. The repository's `.mara/schema.toml` stays a regular file, since loading refuses a symbolic link in the schema path (DES-PROJECT-CONFIGURATION); a host test compares it, read-only, with what `project init` writes. The performance corpus schema, `crates/mara/tests/performance/schema.toml`, declares the same vocabulary with shorter descriptions and can then load the template's schema instead.

## Summary of Changes

- `project init [--template engineering|empty]` (`crates/mara/src/cli/init.rs`, registered in `cli.rs` after `project validate`). It initializes the working directory or the one `--project` names, creating it and its missing parents. It refuses a 0.3.0 root (`legacy-project`), a `.mara` link (`symbolic-link`) and any existing target or a `.mara` that is no directory (`existing-file`, one diagnostic per file at line 1), all before it creates anything. Then it creates `.mara` if absent, takes the write lock and writes the files through `write::apply` with `Basis::Absent`, the configuration last. A failure before the first rename removes the directories it created that are empty again, while it still holds the lock. Result: `{"format_version":1,"created":[".mara/schema.toml",".mara/config.toml"]}`; the human form prints `created <path>` lines.
- Templates embedded with `include_str!`: `crates/mara/templates/config.toml` (both templates), `crates/mara/templates/engineering/schema.toml` (the same text as this repository's `.mara/schema.toml`), `crates/mara/templates/empty/schema.toml`.
- The performance corpus loads the template's schema. `crates/mara/tests/performance/schema.toml` is deleted.
- Tests, `crates/mara/tests/init.rs`, through the real CLI:
  - Both templates, in the working directory, in an existing named directory and in a missing one: the output lists exactly the files created, and `project validate` reports no diagnostic.
  - The written schema equals the repository's `.mara/schema.toml`, read-only, and the configuration equals the DES-PROJECT-CONFIGURATION example.
  - `schema list` and `schema get` match DES-ENGINEERING-VOCABULARY, parsed from its page (types, prefixes, descriptions, fields, values, targets, constraints, `Implements`), and nothing else.
  - The statuses Draft, Accepted and Retired are valid, and another status is `invalid-value`.
  - `empty` declares nothing and validates clean.
  - Refusals change nothing (names, bytes, mtimes and directories): an existing configuration, schema, both, `.mara` as a file, a dangling link, a directory at a target, an existing file in a named directory, 0.3.0, a `.mara` link, a named directory that is a file, an undeclared option and an unknown template.
  - With the debug pause points, with and without an existing `.mara`: the order is step 2, then the schema, then the configuration. The first or later file created at the step-2 pause is refused with `changed-file`, and so is the first file created at its step-3 pause, with nothing replaced and no temporary file left. A failure before the first rename (`.mara` made read-only at the pause) leaves the tree as it was. Undoing the directory cleanup makes that case fail.
- `crates/mara/tests/performance/init.rs`:
  - `project_init_counts`: the VER-WORK-COUNTERS row, all counts 0, both templates, at 1×, 4× and 16×.
  - `project_init_growth_per_doubling`: VER-SCALING on the release build.
  - Each run initializes a new directory outside the corpus from the corpus root, so there is nothing to restore. `scaling::check_growth` is now `pub`.
- Docs:
  - New page `docs/init.mara.md` with DES-PROJECT-INIT (`docs/project.mara.md` is at 194 of 200 lines).
  - `existing-file` added to DES-OPERATIONS.
  - DES-PROJECT-CONFIGURATION's init bullet now links DES-PROJECT-INIT.
  - DES-WRITE-PROCEDURE has one bullet on creating `.mara` before step 1.
  - The index has a row for the new page, and the README status mentions `project init`.

Checks, all exit 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`
- `cargo test --workspace --locked`
- `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings)
- `cargo test --package mara --test performance --release --locked` (12 passed, 2 ignored, load about 6)
- `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`
- `zban check`

## Decisions made

- The directory to initialize is the working directory or the one `--project` names, and `--template engineering|empty` names the template. A missing named directory is created with its missing parents, as 0.3.0 did (`create_dir_all`). Symbolic links in its path are resolved, as selection resolves a root. Rejected: a positional directory argument (every positional argument of the CLI is required, and `--project` already names the directory, as 0.3.0's did); refusing a missing directory.
- Write order: schema, programs (alpha.3), configuration last, as 0.3.0 wrote its project file last. Until the configuration exists, no operation selects the directory, so a stopped init leaves no half project. A rerun refuses the files already written. Rejected: configuration first, which would leave a selectable project without a schema.
- Lock order (DES-PROJECT-INIT, DES-WRITE-PROCEDURE): all refusals are checked first, without the lock. Then `.mara` is created if absent, the lock is taken on it, and `write::apply` checks each target again as absent at steps 2 and 3. A failure before the first rename removes the directories init created that are empty again, last first, before the lock is released. Rejected: locking the root while `.mara` is absent (other writers lock `.mara`, so nothing would be serialized); taking no lock (two inits could both rename).
- New failure code `existing-file` (DES-OPERATIONS). The message lists each existing target, and each gets a diagnostic at its line 1. A `.mara` that is no directory counts as an existing file. Rejected: `changed-file`, whose message says to run the operation again, which would repeat the refusal.
- A 0.3.0 root (`.mara/project.toml` without `.mara/config.toml`) is refused with `legacy-project`, since DES-PROJECT-CONFIGURATION says every operation but the conversion refuses it.
- The configuration of both templates is the DES-PROJECT-CONFIGURATION example (`include = ["docs/**/*.mara.md"]`). Rejected: 0.3.0's `**/*.mara.md`, since the docs come first.
- The `empty` schema is `format_version = 1` and `statuses = ["draft", "accepted", "retired"]`: `statuses` is required and lists at least one name (DES-SCHEMA-MODEL). The engineering statuses are the natural default.
- The result carries `created` only, in write order, with paths relative to the project root, and no root path.
- The performance cases of init live in their own module. Each run writes a new directory, so the harness needed no restore step and `cli::cases` is unchanged.
