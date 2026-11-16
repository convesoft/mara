---
title: "Evaluate programs: execution, invariants in validation, limits and faults"
status: done
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-n3nx6d
  - mara-vt2ktq
  - mara-y4asgm
  - mara-wrbjte
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T17:24:31Z
---

Goal: `project validate` evaluates a project's invariants with Soufflé, safely: only admitted programs run, in a private directory, within limits, and every failure is reported.

Split by the orchestrator on 2026-11-16 into three issues, so that each has one outcome and one reviewable change: this one (execution and invariants), the query operations (see "Blocks"), and explanations. The other two build on what this one exposes.

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-PROGRAMS-ONLY-COMPUTE, DES-RULE-CONSUMERS, DES-REQUEST-INPUTS); docs/rules-facts.mara.md (DES-PROGRAM-PROFILE, DES-FACT-BRIDGE, RISK-SOUFFLE-REACHES-OUTSIDE); docs/rules-evaluation.mara.md (REQ-PROGRAM-FAULTS-REPORTED, REQ-EVALUATION-TIME-LIMIT, DES-RULE-EXECUTION, EVD-SOUFFLE-PROGRAM-REACH). The rows of EVD-SOUFFLE-PROGRAM-REACH are reproductions from independent reviews; each one becomes a test, here for invariants and in the query-operations issue for queries and gates.

Built before this issue: the parser that admits programs (`mara_engine::program`, mara-wrbjte), the fact bridge (`Snapshot::facts`, `MARA_DL`, `facts::check_inputs`, `rules::write_facts`, `rules::supplied_input`, mara-y4asgm), and the pinned Soufflé binary (`scripts/build-souffle.sh`, mara-vt2ktq).

- [x] The configuration entries that declare programs and their consumers (DES-RULE-CONSUMERS) are loaded and checked for every kind of consumer: invariants, queries, views and gates. This issue evaluates invariants; the other kinds are evaluated by the query-operations issue
- [x] Only programs that the parser admits run, on the parsed copies in a private directory (DES-PROGRAM-PROFILE)
- [x] Generated wrapper with JSON `.input` and `.output` directives: every fact relation and supplied input, and every relation that a configuration entry names in the program; Soufflé runs in the private directory with `-l ""` (DES-RULE-EXECUTION)
- [x] Child-process evaluation keyed by the layers read, the program with its includes, and the supplied inputs; every consumer of a program reads the same run. Expose it so that the query operations and explanations can call it
- [x] Invariants in validation: `project validate` and `item validate` report their rows as diagnostics of the declared severity
- [x] Time limit (REQ-EVALUATION-TIME-LIMIT)
- [x] Limits on the output size and the memory of an evaluation: exceeding one ends the evaluation and is reported as a fault, never a passing outcome (REQ-PROGRAM-FAULTS-REPORTED). The worker chooses the numbers and the mechanism, records them in DES-RULE-EXECUTION, and removes the limits part of the open question on docs/rules-facts.mara.md
- [x] Compile errors, refused programs, timeouts, runtime faults and anything Soufflé prints on stderr while evaluating are reported; never a passing outcome
- [x] Soufflé names an included file in a diagnostic only by its base name (`in file b.dl` for `sub/b.dl`, observed 2026-11-16): map each diagnostic to its project path also when two included files share a base name, record how in DES-RULE-EXECUTION, and test it with VER-PROGRAM-FAULTS's included-program case
- [x] A standing test compiles every probe fixture whose `parser` is `admitted` and every program in `crates/mara-engine/tests/programs/trial/` with the pinned `souffle`: a run against empty facts, with a wrapper that reads and writes the fixture's boundary relations as JSON, exits 0 with no compile diagnostic (VER-ADMITTED-LANGUAGE)
- [x] Tests, natively: the invariant cases of VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES (docs/verification-programs.mara.md) and of VER-PROGRAM-CONSUMERS (docs/verification-rules.mara.md) pass; their query, gate and `query report` cases belong to the two issues this one blocks, and their browser rows wait for mara-xsm8w6
- [x] The alpha.3 cold rows of VER-WORK-COUNTERS for validation with programs pass, and VER-SCALING covers it (DES-PERFORMANCE-HARNESS)

Decided by the orchestrator on 2026-11-16 (from mara-t2qrcb, under the maintainer's rule of that day; listed in mara-46j65e), and confirmed for the sandbox by the maintainer: an evaluation gets limits on output size and memory besides time. A finite program wrote 8 MB in 15 ms in a review probe. Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT. The sandbox half of that question is mara-ptq6xc.

## Summary of Changes

- Configuration (`mara_engine::config`, `config/rules.rs`): `[rules]` (`programs`, optional `time_limit_seconds`) and the entries `[queries.*]`, `[views.*]`, `[invariants.*]`, `[gates.*]` are loaded and checked for every kind; the DES-RULE-CONSUMERS example loads as written (unit test).
- Inventory (`mara::inputs`): files that a `programs` pattern matches are inputs, read and hashed like documents, in `Generation::programs`.
- Engine `evaluation` module (no I/O): `plan` admits programs, selects what runs (every entry-named or not-included declared file for `project validate`; programs used by invariants for `item validate`) and checks each entry against its program (`undeclared-relation`, `invalid-relation`); private copies `p<n>.dl` with include paths rewritten to the copies the parser resolved (`Program::includes`); the JSON wrapper; `key`; `interpret` (compile diagnostics, also multi-line and continuation blocks, runtime faults with location, memory exhaustion, anything printed); `read_output` (UTF-8, columns, i32); `Fault::diagnostics` at project paths and lines with source line and caret, or at the declaring configuration line; `invariant_findings`.
- Host `mara::rules`: `souffle()` locator, `Run`/`evaluate`/`Evaluations` (the one spawn place `spawn_and_wait`: empty environment, null stdin, stdout/stderr to files, `RLIMIT_AS` 1 GiB, `RLIMIT_FSIZE` 64 MiB, no core, kill at the time limit), and `invariants` for validation. `project validate` and `item validate` report rows of invariants with the declared severity at the item, and every fault; result keys cover program files and evaluation keys.
- Tests: `crates/mara/tests/programs/` (33 tests): invariant cases of VER-PROGRAM-CONSUMERS, VER-PROGRAM-FAULTS (five compile errors in the program and in an included file sharing its base name with another, refusal with 0 spawns, runtime faults, non-UTF-8 output, lenient functions, output and memory limits), VER-EVALUATION-TIME-LIMIT (1 s, default 10 s, the two endless programs; no `souffle` left, `TMPDIR` unchanged), every VER-PROGRAM-REACH-CASES row with a `cc`-built `libfunctors.so`, a secret file and file snapshots; the standing VER-ADMITTED-LANGUAGE check of all 126 admitted probes and the five trial programs through `mara::rules::evaluate`, comparing rows with the recorded native outputs. Engine unit tests for config and evaluation. Performance harness: the corpus declares a stand-in program with one invariant, query, view and gate; validation cases count 1 program run and 1 spawn; scaling passes.
- Docs: DES-RULE-EXECUTION, DES-RULE-CONSUMERS, DES-SOUFFLE-BUILD, VER-PROGRAM-CONSUMERS setup line, open question in docs/rules-facts.mara.md (limits part removed); the two Evidence items moved to the new page docs/rules-evidence.mara.md (with `item move`), index updated.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo test --package mara --test performance --release --locked`, `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0.

## Decisions made

- Soufflé is found beside the `mara` binary, else at `../souffle-2.5/souffle` from it (Cargo's `target/`), never on `PATH`; packaging stays with mara-a4m588 (DES-SOUFFLE-BUILD).
- Limits: memory 1 GiB of address space (`RLIMIT_AS`, Soufflé fails with `std::bad_alloc`), output 64 MiB per written file (`RLIMIT_FSIZE`, `SIGXFSZ`), no core file; fixed, not configurable. Time limit `[rules] time_limit_seconds`, 1–3600, default 10 (DES-RULE-EXECUTION, DES-RULE-CONSUMERS).
- Base-name mapping: private copies are named `p<n>.dl` and include paths in them are rewritten to the copy the parser resolved, so each diagnostic names one file and Soufflé reads exactly what the parser read.
- A Soufflé compile warning (such as a relation without rules or facts) is a compile error (`program-error`): DES-DIAGNOSTICS allows warnings only from invariants. Codes: `program-refused`, `program-error`, `program-fault`.
- What `project validate` runs: each declared file that an entry names or that no other declared file includes, once; with facts when an invariant uses it, else against empty facts. `item validate` runs the programs invariants use, with every consumer of each program shaping the run, and reports their faults as the item's. Validation supplies each listed input empty and no `source_complete` row.
- Entries: names `[a-z][a-z0-9_-]*`, unique across the four tables; `inputs` optional, none on invariants; an invariant's `description` defaults to its `message`; gate `subjects` and `conditions` non-empty, `unavailable` optional; entry tables without `[rules]` are `missing-key`; a missing entry program is `io-error` at its line. A row about no item's MID is reported at the relation's configuration line.
- Invariant names become diagnostic codes through `diagnostic::intern`, so `Diagnostic::code` stays `&'static str`.

## Rejected approaches

- A four-fold `count` without a condition as the long program: Soufflé materializes the aggregate body and hits the memory limit first; the test adds a condition no row meets.
- Compile warnings as warning diagnostics: contradicts DES-DIAGNOSTICS.

Follow-ups: mara-1znn2z, mara-sqgta5 (backlog).

## Review fixes

The review of `7e3bc82` found six points; each fix has a test that fails without it.

1. An interrupted `mara` left Soufflé running and the private directory behind. The child now gets `PR_SET_PDEATHSIG` = SIGKILL (Linux, set with its limits in `spawn_and_wait`'s `pre_exec`, with a `getppid` check for a parent that ended first), so no Soufflé outlives `mara`, SIGKILL included. SIGINT, SIGTERM and SIGHUP during an evaluation are deferred with `signal-hook` (new dependency, unix only, no `unsafe`): the poll loop kills the child, the private directory is dropped, then `mara` ends by the signal's default action; outside an evaluation they act as by default. DES-RULE-EXECUTION says what happens after SIGKILL. Test: `programs::interrupt` (CLI with SIGTERM, SIGINT, SIGHUP and SIGKILL; `mara mcp` with SIGTERM and SIGKILL).
2. An include cycle hid its compile errors. `project validate` now also runs, in path order, each declared file that no run so far reads, so every declared file is read by a run or refused (DES-RULE-EXECUTION, `Roots::All`). Test: `consumers::an_include_cycle_is_compiled`, which also checks an entry naming the other file of the cycle (1 run, rows reported).
3. `item validate` passed with an invalid invariant entry. It now reports the configuration's diagnostics with the item's (DES-DIAGNOSTICS, VER row in docs/verification-documents.mara.md updated; `seeded::item_validate_reports_the_problems_of_one_item_only` now expects the seeded configuration problem). Test: `consumers::item_validation_reports_the_configuration_problems` (invalid severity, unknown key, missing key, invalid name, undeclared program path).
4. The caret moved after an `.include` on the same line. `original_offset` maps the copy's column back through the rewritten include paths. Test: `faults::the_caret_is_placed_after_an_include_on_the_same_line`.
5. An invalid or taken entry name hid the entry's other problems. The entry is still checked, then left out (DES-RULE-CONSUMERS). Test: `config::rules::tests::checks_an_entry_whose_name_is_invalid_or_taken`; `reports_each_problem_of_an_entry_at_its_line` expects the newly reported problems.
6. No code change; recorded below.

### Decisions made in the review round

- Decided by the orchestrator: `undeclared-relation` and `invalid-relation` are found only against the admitted program, so they are reported by validation and fail the operations that evaluate the entry, as a compile error does, and do not refuse the project to operations that evaluate nothing, such as `item list` and `project fix` (DES-RULE-CONSUMERS).
- Interrupts use `signal-hook` rather than hand-written handlers: no `unsafe`, and its conditional default keeps every signal's default action outside an evaluation.
- `item validate` reports every diagnostic of `.mara/config.toml`, not only those of invariant entries: any of them may drop or change an entry that judges the item.
