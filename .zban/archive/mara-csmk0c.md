---
title: Build the performance harness
status: done
type: task
parent: mara-t1rffa
blocked_by:
  - mara-0ztabc
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T14:13:08Z
---

Goal: the work-counter and scaling tests exist before the first operation, so every operation ships with them.

Context: docs/performance.mara.md (DES-PERFORMANCE-HARNESS, REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING); docs/verification.mara.md (VER-WORK-COUNTERS, VER-SCALING). The audit tools named in EVD-PREVIOUS-IMPLEMENTATION-AUDIT show a working scale-corpus generator for the old format.

- [x] Seeded corpus generator for 1×, 4×, 16× and 64×, writing into temporary directories
- [x] Work counters in engine and hosts, exposed by a debug stats option
- [x] Counter test and scaling test as ordinary Rust integration tests, part of the local checks (CI waits for mara-4dr2p2)
- [x] `[profile.profiling]` as described under Profiling

## Summary of Changes

- Engine: `mara_engine::work::WorkCounts`, the eight counters of DES-PERFORMANCE-HARNESS with `fields()` (name and value, fixed order). Nothing increments them yet: each issue that adds counted work counts it there.
- CLI: `mara` depends on `mara-engine`. `--debug-stats` writes one last stderr line, `{"format_version":1,"file_reads":0,...}`, after the request, also when its arguments are rejected (`crates/mara/src/debug_stats.rs`). Any other argument is rejected with a diagnostic and exit status 2 (REQ-CLI-MCP-PARITY), until the first operation issue brings real argument parsing.
- `crates/mara/tests/performance/`, one integration test target:
  - `corpus.rs` and `schema.toml`: seeded, deterministic generator. 1× = 8 clusters of 25 items in 3 documents plus 12 Rust files with `@mara implements` markers; 200 items, 24 documents, chain depth 2 and fan-out 2 at every size. Each corpus also holds `.git`, `.gitignore`, ignored build output, an ignored document, a document outside the include patterns and a README. Written under `$TMPDIR` with `tempfile`.
  - `cli.rs`: runs the real binary with `--debug-stats` and an empty environment, asserts the case's exit status, and validates the stats line as JSON against its own list of counter names.
  - `work_counters.rs`: `cold_counts` at 1×, 4× and 16×; `cold_counts_64x` is `#[ignore]`d for the nightly run.
  - `scaling.rs`: `growth_per_doubling` runs only in a release build. It does one warm-up round, then five interleaved rounds, and checks the median growth between adjacent sizes against 2.3× per doubling.
  - Until the first operation, the counter test runs `mara --debug-stats` (exit 0) and `mara --debug-stats --no-such-option` (exit 2); neither may do work. The scaling test runs the first, which must not grow.
- `[profile.profiling]` (inherits `release`, `debug = "line-tables-only"`) in the workspace `Cargo.toml`.
- AGENTS.md local checks gain `cargo test --package mara --test performance --release --locked` (108 lines).
- DES-PERFORMANCE-HARNESS records the decisions below.

Checks:

- All six local checks exit 0: fmt, clippy with warnings denied, `cargo test --workspace` (5 performance tests: 3 pass, scaling and 64× ignored in debug), the release performance test (4 pass, 64× ignored), the `wasm32` engine build and `zban check`. The 64× test passes with `-- --ignored` (0.3 s).
- Generated 1× corpus inspected by hand against docs/format.mara.md and docs/identity.mara.md: anchors, separators, metadata tables, relative relation links across files and areas, mentions, and markers.
- Seeded faults in a scratch copy under `$TMPDIR`: a binary that reads every `*.mara.md` fails the counter test (25 reads: the 24 documents plus the ignored one). Quadratic work fails the scaling test (2.89× per doubling). A stats line without `program_runs` fails with "program_runs is missing". Linear work passes at 1.93–1.97× per doubling in three runs.
- `cargo build --profile profiling` emits `.debug_line`.
- Every link added to docs and AGENTS.md resolves to its MID with the link text as ID.

## Decisions made

- One `WorkCounts` per request, passed by `&mut` to engine calls; the code that does the work counts it. Rejected: global or thread-local counters, which would mix concurrent daemon requests.
- A check step is one item or edge visited by a built-in check, so the REQ-NEAR-LINEAR-SCALING bound can be checked as steps ≤ k·(items + edges).
- `--debug-stats` is a flag; its output is the last stderr line, after any diagnostic, also when the request fails or its arguments are rejected. The line is a JSON object of exactly `format_version` 1 and the eight counters by name (`file_reads`, `hashes`, `parses`, `snapshot_builds`, `code_scans`, `subprocess_spawns`, `check_steps`, `program_runs`); key order carries no meaning. Rejected: stdout, which `--format json` owns; a file path argument; an environment variable.
- Generator and tests are one integration test target, `performance`, in the `mara` crate, so they share helpers without dead code. Rejected: a separate crate or a generator binary, which the docs do not ask for.
- A seed and a size give byte-identical files everywhere: seeded SplitMix64 written in the test, not `rand`, whose generators may change between versions; one stream per cluster; MIDs from a fixed timestamp plus the item number. `tempfile` is a dev-dependency for the temporary directories.
- Corpus shape: 8 clusters per 1×, each linking only to itself and to the previous cluster, so 1× is a prefix of every larger corpus. Chains have depth 2 and fan-out 2 at every size.
- The corpus uses the configuration example of DES-PROJECT-CONFIGURATION and a schema with only the keys DES-SCHEMA-MODEL shows (no `required`, `repeatable`, same-type or acyclic keys yet). Each Verification and Validation has one method and each Term one alias. Code languages and programs are not declared until their configuration syntax exists.
- Term titles and aliases carry the item index and differ in word count, so no two accepted Terms collide.
- Scaling: a release build only (`cfg_attr(debug_assertions, ignore)`), one warm-up round and five timed rounds that interleave sizes, median per size, growth per doubling = (t₂/t₁)^(1/log₂(s₂/s₁)) between adjacent sizes.
- 64× is an ignored test that asserts counts only: REQ-NEAR-LINEAR-SCALING bounds growth from 1× to 16×.
- A case is the arguments and the exit status of one invocation; the harness asserts the status before it accepts counts or timing. The tests validate the stats line as JSON (`serde_json`, dev-dependency) against their own list of counter names, never against `WorkCounts`. Two cases without an operation pin today's behaviour and stay: `mara --debug-stats` exits 0, and `mara --debug-stats --no-such-option` exits 2 after a diagnostic; neither does work. Each operation adds its case to `cold_counts_at` and `growth_per_doubling` (mara-000f9q, mara-bmc9gc). mara-t6xn9z counts parses and mara-8d6cnw counts file reads, hashes and snapshot builds in `WorkCounts`. The code-links and rules slices declare their inputs in the generated configuration.

Filed: mara-63dn2d. mara-8d6cnw must assert the cold rows through the real CLI, but no CLI operation exists before mara-000f9q and mara-bmc9gc, which it blocks.

## Review fixes

An independent review found four defects and one minor finding. All five are fixed in a second commit on the branch:

1. Exit status: `cli::run` takes the expected exit status and asserts it before it parses the counts. The counter test and the scaling test give one for each case. The case without an operation pins exit 0, which is what `mara` does today.
2. Stats contract: `cli.rs` parses the line as JSON and checks for exactly `format_version` 1 and the eight counter names, which the test lists itself. It builds `WorkCounts` field by field and no longer uses `WorkCounts::fields()`.
3. Argument errors: `main.rs` reads every argument first. It still writes the diagnostic and exits 2, then writes the stats line when `--debug-stats` was given, in either argument order. A new case runs `mara --debug-stats --no-such-option` and asserts exit 2, no work, and the diagnostic before the stats line.
4. mara-63dn2d is tagged `maintainer` and blocks mara-8d6cnw. Nothing else changed on mara-8d6cnw, mara-000f9q or mara-bmc9gc.
5. DES-PERFORMANCE-HARNESS now holds every lasting choice from the list above: the stats line's place and keys, how a seed reproduces a corpus, the cluster and chain bounds, the schema limits, exit status per case, the pinned cases, the independent check of counter names, and the growth formula. The page is 145 lines.

Checks: the faults were planted again in a scratch copy under `$TMPDIR`, one at a time, each on a clean copy. Each of these fails the release performance test:
- exit 2 on success (fails both tests)
- `program_runs` removed from `WorkCounts::fields()` (both tests)
- JSON quotes stripped (both tests)
- a renamed counter (both tests)
- no stats line on an argument error (counter test)
- reads of every document (counter test, 25 reads)
- quadratic work (scaling test, 2.86× per doubling)

Linear work still passes. All six local checks exit 0 at the final commit.
