---
title: The checks that guard this repository's docs are a project of their own, and mara:test no longer reads docs/
status: todo
type: task
parent: mara-c8scn7
created_at: 2026-11-22T19:14:52Z
---

Goal: a change to a page under `docs/` or to `.mara/` reruns only the checks that guard those files, in seconds, and the test suites of `mara` and `mara-engine` are restored from the cache.

Context: decided by the maintainer on 2026-11-22: the tests that read this repository's docs "are not source code tests"; they are like doctests of the documentation and belong beside `mara:validate`. Today `mara:test` declares all of `docs/` and `.mara/` as inputs and both crates declare five pages as `fileInputs` (`ADR-NX-RUST-TASKS`, `crates/mara/Cargo.toml`, `crates/mara-engine/Cargo.toml`), and nearly every change of this project edits a page, so nearly every change reruns all 823 tests. nx-rust 1.0.0-rc.2 gives every crate a `test` target with inputs of its own, and a configuration of a target cannot change its inputs (`node_modules/@convesoft/nx-rust/docs/configuration.mara.md`, "Changing an inferred target"; `docs/caching.mara.md`, "Files outside the crate"), so the way that keeps the plugin's keys is a small crate that is never published.

The checks in question:

- Documented examples load in the real code: tests in seven places take examples from `docs/format.mara.md`, `docs/rules/index.mara.md`, `docs/project.mara.md`, `docs/code/index.mara.md` and `docs/template/vocabulary.mara.md` with `include_str!` (`crates/mara-engine/src/document/tests.rs`, `src/facts/tests.rs`, `src/config/rules.rs`; `crates/mara/tests/init.rs`, `tests/project.rs`), among them that `mara.dl` is the listing on the rules page and that the vocabulary page's tables equal what `schema get` returns
- The repository uses the template unchanged: `crates/mara/tests/init.rs` compares what `project init` writes, which is `crates/mara/templates/engineering/`, with this repository's `.mara/schema.toml` and rule programs
- The parser loses no item of the real pages: `crates/mara/tests/documents.rs` walks `docs/` at run time, also with lone-CR line endings

- [ ] A crate in the workspace that is never published (`publish = false`, outside every package the release builds) holds these checks; its `test` target has `docs/` and `.mara/` among its inputs, declared the way the plugin's docs say for files outside a crate and for files read at run time
- [ ] Each check still fails when what it guards breaks: shown once per check by a temporary edit of the page or file (an example made invalid, a table row changed, a rule program of `.mara/` edited, an anchor damaged), reverted afterwards
- [ ] What a moved check needs from `mara-engine` or `mara` is public API that exists or a minimal addition; a check whose code half tests the product (for example that `project init` writes its files and the result validates) stays in its crate without the comparison to the repository
- [ ] `mara:test` no longer declares `docs/` and `.mara/`, and the crates no longer declare the pages as `fileInputs`, wherever no remaining test or macro reads them: shown by a search for the reads and by probes on a private cache (a docs page edited: the new project's `test` runs, `mara:test` and `mara-engine:test` are restored; a source file of `mara-engine` edited: all three run)
- [ ] `pnpm run checks` and `pnpm run checks:fast` run the new project's tests without a change to the scripts, or the scripts change with it; the total of passed tests is the same as before (855 passed, 0 failed, 16 ignored), and the closing summary says how many moved
- [ ] `ADR-NX-RUST-TASKS` records the decision with its rejected alternatives (a hand-written target, which cannot reuse the plugin's keys; a configuration of `mara:test`, which shares its inputs; fixtures copied from the pages, which let a page drift), and the table names the new target

Not part of this issue: the test runner and affected selection (the nextest issue, which this one blocks), `mara:validate`.
