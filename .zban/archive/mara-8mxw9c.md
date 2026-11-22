---
title: The checks that guard this repository's docs are a project of their own, and mara:test no longer reads docs/
status: done
type: task
parent: mara-c8scn7
created_at: 2026-11-22T19:14:52Z
closed_at: 2026-11-22T20:00:39Z
---

Goal: a change to a page under `docs/` or to `.mara/` reruns only the checks that guard those files, in seconds, and the test suites of `mara` and `mara-engine` are restored from the cache.

Context: decided by the maintainer on 2026-11-22: the tests that read this repository's docs "are not source code tests"; they are like doctests of the documentation and belong beside `mara:validate`. Today `mara:test` declares all of `docs/` and `.mara/` as inputs and both crates declare five pages as `fileInputs` (`ADR-NX-RUST-TASKS`, `crates/mara/Cargo.toml`, `crates/mara-engine/Cargo.toml`), and nearly every change of this project edits a page, so nearly every change reruns all 823 tests. nx-rust 1.0.0-rc.2 gives every crate a `test` target with inputs of its own, and a configuration of a target cannot change its inputs (`node_modules/@convesoft/nx-rust/docs/configuration.mara.md`, "Changing an inferred target"; `docs/caching.mara.md`, "Files outside the crate"), so the way that keeps the plugin's keys is a small crate that is never published.

The checks in question:

- Documented examples load in the real code: tests in seven places take examples from `docs/format.mara.md`, `docs/rules/index.mara.md`, `docs/project.mara.md`, `docs/code/index.mara.md` and `docs/template/vocabulary.mara.md` with `include_str!` (`crates/mara-engine/src/document/tests.rs`, `src/facts/tests.rs`, `src/config/rules.rs`; `crates/mara/tests/init.rs`, `tests/project.rs`), among them that `mara.dl` is the listing on the rules page and that the vocabulary page's tables equal what `schema get` returns
- The repository uses the template unchanged: `crates/mara/tests/init.rs` compares what `project init` writes, which is `crates/mara/templates/engineering/`, with this repository's `.mara/schema.toml` and rule programs
- The parser loses no item of the real pages: `crates/mara/tests/documents.rs` walks `docs/` at run time, also with lone-CR line endings

- [x] A crate in the workspace that is never published (`publish = false`, outside every package the release builds) holds these checks; its `test` target has `docs/` and `.mara/` among its inputs, declared the way the plugin's docs say for files outside a crate and for files read at run time
- [x] Each check still fails when what it guards breaks: shown once per check by a temporary edit of the page or file (an example made invalid, a table row changed, a rule program of `.mara/` edited, an anchor damaged), reverted afterwards
- [x] What a moved check needs from `mara-engine` or `mara` is public API that exists or a minimal addition; a check whose code half tests the product (for example that `project init` writes its files and the result validates) stays in its crate without the comparison to the repository
- [x] `mara:test` no longer declares `docs/` and `.mara/`, and the crates no longer declare the pages as `fileInputs`, wherever no remaining test or macro reads them: shown by a search for the reads and by probes on a private cache (a docs page edited: the new project's `test` runs, `mara:test` and `mara-engine:test` are restored; a source file of `mara-engine` edited: all three run)
- [x] `pnpm run checks` and `pnpm run checks:fast` run the new project's tests without a change to the scripts, or the scripts change with it; the split adds one pass (856 passed, 0 failed, 16 ignored), and the closing summary says how many moved
- [x] `ADR-NX-RUST-TASKS` records the decision with its rejected alternatives (a hand-written target, which cannot reuse the plugin's keys; a configuration of `mara:test`, which shares its inputs; fixtures copied from the pages, which let a page drift), and the table names the new target

Not part of this issue: the test runner and affected selection (the nextest issue, which this one blocks), `mara:validate`.

## Summary of Changes

- New crate `crates/docs-checks` (`publish = false`, tests only, the library has no code and no dependency; `mara`, `mara-engine`, `serde_json` and `tempfile` are dev-dependencies). Nothing depends on it and no package holds it. Package-scoped release and Wasm builds select the requested product crates. Nx's host release-profile build compiles all workspace members in a staging directory and retains the requested crate's artifacts. Its tests read the pages, `.mara/` and `crates/mara/templates/` at run time from the checkout. Its `test` target adds `{workspaceRoot}/docs/**/*`, `{workspaceRoot}/.mara/**/*`, `{workspaceRoot}/crates/mara/templates/**/*` and `TMPDIR` to the plugin's inputs (`"..."` kept). It neither starts the `mara` binary nor runs Soufflé, so it needs neither `CI`, `MARA_NO_DAEMON`, the `MARA_TEST_*` hooks nor the dependency on `souffle`.
- 6 tests moved, with their bodies and only API adaptations (no `@mara` markers were on them):
  - from `mara-engine`: `document::tests::reads_the_documented_example`, `facts::tests::mara_dl_is_the_version_that_des_fact_bridge_shows` and `config::rules::tests::loads_the_example_of_the_design`, now in `tests/examples.rs`. The only change is `Diagnostic` compared whole instead of through the crate-private `built_in_code`.
  - from `mara`: `tests/documents.rs` (moved as-is), and `the_documented_examples_load_as_a_project` and `the_documented_code_example_loads` from `tests/project.rs`, now in `tests/examples.rs`, still through `mara::project::{select, load}`.
- 1 test split, as the issue's third item asks. The repository comparisons of `tests/init.rs` (`.mara/` equals the template; both templates' configurations start with the example of DES-PROJECT-CONFIGURATION; the vocabulary page equals what `schema get` returns) are the new `docs-checks` test `the_engineering_template_is_this_repositorys_project_and_its_vocabulary_page`. It uses the engine's public `schema_list` and `schema_get` on the template's schema. The product halves stay in `init.rs`: what `project init` writes equals the template files (`include_str!` inside the crate). `schema list` and `schema get` of the initialized project, through the real CLI, return exactly what the engine returns for the template's schema, so VER-PROJECT-INIT keeps its real-CLI case.
- No addition to `mara-engine` or `mara`: everything the moved checks use was public already.
- Removed: the `fileInputs` of both crates, and `docs/**` and `.mara/**` from `mara:test`'s inputs.
- `ADR-NX-RUST-TASKS`: the Cache keys bullet no longer lists `docs/` and `.mara/` for `mara:test`. The ADR records the decision and its rejected alternatives. The table names `docs-checks:test`'s inputs and the `fmt-check` command with `-p docs-checks`.

## Evidence that no read remains

- Search: `include_str!`, `include_bytes!`, `CARGO_MANIFEST_DIR` and relative `../` paths under `crates/` (no symbolic links exist under `crates/`). What remains in `mara` and `mara-engine` reads only inside each crate (`templates/`, `tests/fixtures`, `tests/programs`, the `catalog.toml` and `guide.md` sources), the Soufflé in `target/` (keyed by the runtime input), `../mara-engine/tests/programs` (a dependency's folder) and the tag `v0.3.0` through `git archive` (keyed by the runtime input).
- Run: with `docs/` and `.mara/` moved aside, `cargo test --offline --locked -p mara -p mara-engine --no-fail-fast` gave 817 passed, 0 failed, 14 ignored, exit 0. Both folders were restored afterwards.

## Each check fails when what it guards breaks

Each probe was a temporary edit, reverted afterwards (`$TMPDIR/probe.sh`; `cargo test -p docs-checks --no-fail-fast`). The failing test(s):

- `docs/format.mara.md`: the example's `Status` value misspelled → `reads_the_documented_example`. The example's anchor MID changed → the same test.
- `docs/rules/index.mara.md`: `format version 1` → `2` in the `mara.dl` listing → `mara_dl_is_the_version_that_des_fact_bridge_shows`. An unknown key added to the `[rules]` example → `loads_the_example_of_the_design`.
- `docs/project.mara.md`: the include pattern of DES-PROJECT-CONFIGURATION changed → `the_documented_examples_load_as_a_project` and `the_engineering_template_is_this_repositorys_project_and_its_vocabulary_page`.
- `docs/code/index.mara.md`: a comment prefix dropped from the code example → `the_documented_code_example_loads`.
- `docs/template/vocabulary.mara.md`: a table row's description changed → `the_engineering_template_is_this_repositorys_project_and_its_vocabulary_page`.
- The same test also fails for each of these: `.mara/rules/knowledge.dl` with a comment line added; `.mara/schema.toml` with a newline added; `crates/mara/templates/engineering/rules/readiness.dl` with a comment line added.
- `docs/engine.mara.md`: the first anchor's MID shortened by one character, or its last character replaced by `u` → `every_anchor_in_docs_is_an_item_with_any_line_ending`.

## Cache probes

Private cache `NX_CACHE_DIRECTORY=$TMPDIR/nxcache.*`. Command: `NX_DAEMON=false NX_USE_LOCAL=true pnpm nx run-many -t test -p docs-checks mara mara-engine --output-style=static`.

- Fresh cache: all three tasks run. Second run: `docs-checks:test`, `mara:test` and `mara-engine:test` each `[existing outputs match the cache, left as is]`.
- A: `Probe.` appended to `docs/engine.mara.md` → `mara:test` and `mara-engine:test` restored, `> nx run docs-checks:test` runs ("read the output from the cache … for 3 out of 6 tasks"). Reverted → all three restored (4 of 6). The same probe with other text took 3 s end to end.
- B: a comment appended to `.mara/config.toml` → the same as A: only `docs-checks:test` runs. Reverted → all three restored.
- C: a comment appended to `crates/mara-engine/src/work.rs` → all three run. Reverted → all three restored.
- D (`nx run docs-checks:test`): `TMPDIR` set to another directory → it runs. Back → restored.
- E: a comment added to `crates/mara/templates/engineering/rules/readiness.dl` → it runs (and fails, as it should). Reverted → restored.

## Checks

- `pnpm run checks:fast`: exit 0. Tasks: `fmt-check`, `lint` and `test` of `docs-checks`, `mara-engine` and `mara`, `mara:validate`, `zban-check`, and the dependencies `souffle`, `lockfile` and `mara:build`. `project validate` 0 errors, 1 warning (the known one). Tests: 824 passed, 0 failed, 14 ignored.
- `pnpm run checks` (under `berth --exclusive`): exit 0, `project validate` 0 errors, 1 warning. Tests: 856 passed, 0 failed, 16 ignored, that is 855 + 1: the 6 moved tests count as before, and the split of the vocabulary check adds the one new test.
- A second run of `pnpm run checks`: the first `nx` command has 14 tasks (11 before: plus `docs-checks`' `fmt-check`, `lint` and `test`) and restores 12 of them (all but `lockfile` and `souffle`). It also restores `mara-engine:build:wasm32` and runs `mara:test:performance` again. Tests 856/0/16.

## Decisions made

- `docs-checks` reads the pages, `.mara/` and the templates at run time and keys them on its `test` target. It does not use `include_str!` with `fileInputs`, which would also run its `build`, `check` and `lint` again after every page edit (recorded in ADR-NX-RUST-TASKS).
- The `test` target of `docs-checks` keys `TMPDIR`, under which its project checks write. It does not key `CI`, `MARA_NO_DAEMON`, the `MARA_TEST_*` hooks or Soufflé, because nothing in it reads them.
- The test count rises by one (856): the vocabulary check of `init.rs` is split into a product half, which keeps VER-PROJECT-INIT's real-CLI case in `mara`, and the comparison with the page, which moved.
- The crate is named `docs-checks` (Nx project `docs-checks`), at `crates/docs-checks`.
