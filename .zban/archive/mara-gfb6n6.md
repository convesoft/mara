---
title: "Query operations: execute, view, gate and list in CLI and MCP"
status: done
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
created_at: 2026-11-16T16:11:48Z
closed_at: 2026-11-16T18:44:33Z
---

Goal: a user or agent runs a project's named queries, views and gates, with supplied inputs, from the CLI and through MCP.

Split by the orchestrator on 2026-11-16 from mara-bx82k0, which builds the execution this issue calls (admitted programs in a private directory, the generated wrapper, the keyed child-process evaluation, limits and fault reporting, invariants in validation, and the configuration entries of every consumer kind). Explanations (`query report`) are a third issue.

Context: docs/rules.mara.md (REQ-PROJECT-AUTHORED-LOGIC, REQ-EXTERNAL-RECORDS-AS-SUPPLIED, DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-REQUEST-INPUTS); docs/rules-evaluation.mara.md (REQ-PROGRAM-FAULTS-REPORTED, DES-RULE-EXECUTION); DES-OPERATIONS and REQ-CLI-MCP-PARITY in docs/hosts.mara.md.

- [x] Named queries, views and gates: `query execute`, `query view`, `query gate` and `query list` in the CLI and as MCP tools
- [x] Gate outcome per named item from subject, condition and unavailable relations (DES-GATE-OUTCOME)
- [x] Request inputs on the command line and through MCP: `--input` inline and from a file, `--complete`; a missing, unlisted or wrong input refuses the request before any run (DES-REQUEST-INPUTS). Use `facts::check_inputs`, `rules::supplied_input` and `rules::write_facts` from mara-y4asgm
- [x] A fault of a program consumed by a query or a gate fails the request or makes the gate and the items it names unavailable; never a passing outcome (REQ-PROGRAM-FAULTS-REPORTED)
- [x] Tests, natively: the query, view, gate and list cases of VER-PROGRAM-CONSUMERS; VER-FACT-ROWS through a configured query and `query execute`; VER-REQUEST-INPUTS through `--input`, `--complete` and a gate: the per-item outcomes (unavailable, failed), the MCP rows, refusals with 0 program runs in `--debug-stats`, the spawn count, and a later request without inputs refused with the project tree unchanged (docs/verification-rules.mara.md); the query and gate cases of VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES (docs/verification-programs.mara.md). Browser rows wait for mara-xsm8w6
- [x] The `query` rows of VER-CLI-MCP-PARITY pass for these four operations
- [x] The alpha.3 cold rows of VER-WORK-COUNTERS for `query execute`, `query view`, `query gate` and `query list` pass, and VER-SCALING covers them (DES-PERFORMANCE-HARNESS)
- [x] The agent guide (`crates/mara/src/cli/guide.md`, DES-AGENT-GUIDE) names the new operations within its size limit, since its tests compare its names with the operations in both directions

What mara-bx82k0 exposes: `mara_engine::config::{Rules, Entry, EntryKind, Consumer}` (every entry kind loaded and checked); `mara_engine::evaluation::plan(rules, Roots, files)` gives each admitted program with its valid entries (`PlannedEntry::inputs` are the `InputDecl`s for `facts::check_inputs`) and the union of their output relations; `mara::rules::{souffle, Run, evaluate, Evaluations}` runs one evaluation keyed by `evaluation::key` and returns `Rows` or a `Fault` (`Fault::diagnostics` places it). `mara::rules::invariants` shows the validation path. The performance corpus already declares a stand-in program with one invariant, query (`verified-requirements`), view (`unverified`) and gate (`verified`, inputs `candidate` and `verification_result(v: Mid, c: symbol, result: symbol)`) in `crates/mara/tests/performance/corpus.rs`. Test helpers: `crates/mara/tests/programs/main.rs` (`Sandbox`).

## Summary of Changes

- Engine: `evaluation::plan` takes `Roots::Entry(index)`, the one program an entry names (a missing program reported for that entry only); `evaluation::judge` gives a gate's outcome per named item and for the gate (DES-GATE-OUTCOME), all unavailable without rows; `read::{query_list, query_rows, gate_page}` write the paged results (DES-QUERY-RESULTS); `EntryKind::name`, `facts::write_row`, `evaluation::compare_rows`.
- Host: `mara::rules::consume` evaluates one entry with its supplied inputs: plan of its program, `check_inputs` against the entry's inputs before any run, inputs only other entries list read empty (the same run validation shapes), one keyed evaluation through `Evaluations`. `spawn_and_wait` is unchanged. `supplied_input` counts the file read.
- CLI and MCP (`crates/mara/src/cli/query.rs`): `query list`, `query execute`, `query view`, `query gate`, with `--id`, `--input`, `--complete`, `--limit`, `--cursor`. `Opt::kind` (`Text`, `Integer`, `Inputs`) replaces `integer`; MCP's `input` is an object from each input's name to a string or a list of rows (`Args::supplied`). `resolve_item` moved to `cli.rs` and shared with `item validate`. Failure codes `unknown-entry`, `program-failed` and the input codes.
- Agent guide: one bullet names the four operations; other bullets shortened so the MCP rendering stays within 3,800 characters.
- Tests: `programs/queries.rs` (VER-PROGRAM-CONSUMERS query, view, gate and list cases, paging, an entry that does not fit, configuration errors), `programs/inputs.rs` (VER-REQUEST-INPUTS through the CLI: rows read as given, fail and missing results with and without `--complete`, refusals with 0 runs, wrong rows, spawns = runs, a later request without inputs refused with the sandbox's files unchanged); `both_severities` adds a query and a gate, so every VER-PROGRAM-FAULTS, VER-EVALUATION-TIME-LIMIT and VER-PROGRAM-REACH-CASES case also runs `query execute` and `query gate` (`assert_unavailable`); VER-FACT-ROWS runs through configured queries and `query execute`; `mcp/parity.rs` runs the query operations through both surfaces, the inputs from a file on the CLI and as rows through MCP; `mcp/protocol.rs` checks the `input` object and the gate's required `id`. Performance: the corpus writes `results.json`; cases `query list` (configuration and schema reads), `query execute`, `query view` and `query gate` (1 run, 1 spawn, + 1 file read for the input file) in both the counter and scaling tests.
- Docs: DES-QUERY-RESULTS (new, docs/rules.mara.md); DES-REQUEST-INPUTS (MCP's `input` object); DES-OPERATIONS codes and DES-MCP-SERVER parameters (docs/hosts.mara.md); DES-PERFORMANCE-HARNESS (`results.json`).

Checks, at the head rebased onto `fa88e93`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo test --package mara --test performance --release --locked` (under `berth --exclusive`), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0.

## Decisions made

- Result format (DES-QUERY-RESULTS): `query` head with `columns`; each row with its `item` node or `null`; gate head `name`/`outcome`, items with `outcome` and the rows that caused it (`relation`, `role`, `values`); outcomes `passed`, `failed`, `unavailable`, `not-applicable`. Results page like reads; a gate's `diagnostics` hold the problems that made it unavailable first.
- `query view` returns the same JSON as `query execute`; only its human form differs (a Markdown table). Rejected: a `table` member in the JSON, which doubles each page's bytes.
- Items are named with the repeatable `--id`, as for `search` and `item list`; an unknown or ambiguous one refuses the request. A gate without `--id` is `invalid-argument` (exit 2) before anything is read.
- An operation runs only entries of its kind; another kind's name is `unknown-entry`, naming the operation that runs it.
- `query execute`/`query view` with a program that gives no rows fail with `program-failed`; a gate is unavailable with exit status 1. A gate exits 0 only when passed.
- MCP `input` is an object (name → string or list of rows); a string is always one value, never a file. Rejected: an array of `<name>=<value>` strings, which cannot pass rows.
- An `--input` file counts as a file read and is not hashed: it is no project input.
- `query list` reads the configuration and schema only and refuses an invalid project, like `schema list`.

## Rejected approaches

- A `table` member in `query view`'s JSON (see above).

## Review fixes

The review of `3f35eaf` found two points.

1. Decided by the orchestrator, from the review: a project whose configuration or schema has an error refuses `query gate`, like every `query` request, as `invalid-project` with the configuration's diagnostics, exit status 1, before any outcome: no `gate`, no `items` (DES-PROJECT-CONFIGURATION). `unavailable` is the outcome of a gate whose evaluation failed. The sentence is in DES-QUERY-RESULTS. Test: `programs::queries::a_configuration_error_refuses_the_query_operations` now also requires exit status 1, the configuration's diagnostics, no `gate`, `items`, `rows` or `entries`, and 0 runs and spawns, for `query list`, `query execute` and `query gate`.
2. A fault diagnostic larger than the 8 KiB budget for a page's diagnostics was dropped from a gate result (`diagnostics: []`, `diagnostics_total: 1`). The problems that make a gate unavailable are now shown on every page whatever their size; only the inventory's diagnostics after them keep the budget (`Envelope::kept`, `shown_diagnostics(diagnostics, kept)`, `respond_keeping` for the human form; DES-QUERY-RESULTS). Test: `programs::queries::a_gate_keeps_a_fault_diagnostic_of_any_size` (a compile error whose source line carries a 9,000-character comment: the gate's JSON and human form show it at its file and line, `query execute` fails with it); it fails without the fix.

### Decisions made in the review round

- Decided by the orchestrator: `invalid-project` refuses a gate request before any outcome; `unavailable` is only the outcome of a failed evaluation.
- A gate's problems are exempt from the diagnostic budget, as a query's failure carries all of its diagnostics; they count toward the page's 64 KiB, and a gate whose problems alone exceed it fails as `record-too-large` naming the gate's configuration line.
