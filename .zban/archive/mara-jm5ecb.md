---
title: "Explanations: query report"
status: done
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
created_at: 2026-11-16T16:11:57Z
closed_at: 2026-11-16T21:19:50Z
---

Goal: `query report` explains one row of an invariant or gate outcome, down to the documents and supplied inputs it rests on, or says that it cannot.

Split by the orchestrator on 2026-11-16 from mara-bx82k0, which builds the execution this issue calls. The query operations (`query execute`, `view`, `gate`, `list`) are a sibling issue; this one needs their command group and the gate outcome only for the gate rows, so it can start on invariants as soon as mara-bx82k0 is integrated.

Context: REQ-EXPLAIN-OUTCOMES and DES-RULE-EXECUTION in docs/rules-evaluation.mara.md; DES-RULE-CONSUMERS in docs/rules.mara.md; VER-EXPLANATIONS in docs/verification-rules.mara.md.

- [x] Explanations from provenance mode (`-t explain`, driven on stdin with `format json`) with document locations: `subproof` references expanded, the returned proof checked against the requested row, constraint leaves left out
- [x] A leaf from a supplied input names that input and holds the row's values as supplied (REQ-EXPLAIN-OUTCOMES as amended; decided by the maintainer on 2026-11-16)
- [x] "Cannot explain" for values the `explain` command cannot write (quotes, backslashes, control characters, negative numbers), invalid replies, a branch that ends in an aggregate, aborts, and the time limit; the outcome stands either way
- [x] `query report` in the CLI and as an MCP tool, for rows of invariants and of gates
- [x] Tests, natively: the cases of VER-EXPLANATIONS, and the `query report` cases of VER-PROGRAM-FAULTS and VER-EVALUATION-TIME-LIMIT (docs/verification-programs.mara.md). The checks that no admitted program can reach (a returned proof that is not for the requested row, a reply that is not JSON, an aborted provenance run) are verified by inspection in this issue's review (decided by the orchestrator on 2026-11-16; no fault-injection hook is added). Browser rows wait for mara-xsm8w6
- [x] The `query report` row of VER-CLI-MCP-PARITY, its cold row of VER-WORK-COUNTERS (1 program run, in provenance mode) and its VER-SCALING case pass
- [x] The agent guide names `query report` within its size limit

What mara-bx82k0 exposes: `mara::rules::evaluate` writes the private copies and wrapper and runs Soufflé through `spawn_and_wait`, the one place Mara starts a process (mara-ptq6xc will wrap it). Provenance mode needs that function to take extra arguments (`-t explain`) and a standard input instead of `Stdio::null()`; the limits, the private directory and `evaluation::interpret` apply unchanged.

## Summary of Changes

- Engine (`mara_engine::evaluation::explain`, no I/O): `START` (`format json`, then `explain __mara_ready()`, whose reply says the evaluation ended), `explain_command` (refuses quotes, backslashes, control characters and negative numbers), and `Explainer`, which gives one command at a time and takes each reply: checks the root against the requested row, expands `subproof` references, leaves out constraint leaves, finds each leaf among the run's facts and supplied inputs by how Soufflé prints the row (several matching rows: cannot explain), places fact leaves (`item`/`edge` at their own path and line, `title`/`revision`/`field` at the item's, `folded` at the first item whose title or field value it is), refuses a proof that passes an aggregate (`: {` in a step's rule, or a leaf of a non-input relation such as `__agg_single`), a reply that is no proof, a stray reply, and proofs over 1,000 steps or 32 KiB. Negated literals with a wildcard are shown as the rule writes them, since Soufflé prints arbitrary values for wildcards. `read::report_page` writes the paged result; `ItemSummary::of` is public.
- Host (`mara::rules`): `spawn_and_wait` takes an optional console. With one it adds `-t explain` and pipes stdin and stdout; the private directory, limits, parent-death signal and signal handling are unchanged. `rules/explain.rs`: `Exchange` writes commands on one thread and reads replies on another, at most 16 MiB (the rest drained and dropped), and the existing poll loop enforces the time limit. `explain` and `consume_explained`: at the first reply the host interprets stderr so far and reads the outputs (a fault there is the evaluation's fault: outcome unavailable), then the caller chooses the rows. After that, the time limit, an abort, an invalid or stray reply, or an overflow leave the outcome standing and the rows not yet explained without an explanation, naming why. `evaluate` now shares `private_dir` and `read_rows`; `consume` is generic over the kind of run.
- CLI and MCP: `query report <name> --id <id-or-mid>` (one, required), `--input`, `--complete`, `--limit`, `--cursor`, for invariants and gates. Exit 0 only when every row is explained. The human form prints the proof one step per line.
- Agent guide: names `query report`. The MCP rendering is 3,778 characters, from shorter wording of the unbound-server note and the closing validate line; no operation dropped.
- Docs: DES-QUERY-RESULTS (row and two bullets: rows, outcome, proof nodes), DES-RULE-EXECUTION (three bullets: the exchange, leaf locations, aggregates and negations and size bounds), DES-MCP-SERVER (`id` of `query report` required), DES-PERFORMANCE-HARNESS (the report case's inputs), VER-EXPLANATIONS (inspection row names mara-jm5ecb).
- Tests: `programs/explain.rs` covers every VER-EXPLANATIONS test row. Leaves are compared with `get` and `relation list`; the input leaf, negation (with and without a wildcard), a record, a component and an `eqrel`; the 8-link closure with no `subproof` left; `x < "z"` without a constraint leaf; newline, backslash, quote, negative number and a quoted title cannot be explained, while the plain row in the same request is explained for its own row; two aggregate forms; validation unchanged by every report and the gate outcome equal to `query gate`'s; argument refusals; a gate fault. `assert_unavailable` also runs `query report` of the invariant, so every VER-PROGRAM-FAULTS and VER-EVALUATION-TIME-LIMIT case covers it, timings included. MCP parity runs the report (failed with `source_complete` and `candidate` input leaves, a broken gate, refusals), and the protocol test lists `query_report` with required `id`. Performance has a `query report` case: 1 run, 1 spawn, plus 1 input file read. Engine unit tests cover a proof of another row, subproof expansion, a non-proof or stray reply, an aggregate, an ambiguous leaf, and rows the command cannot take.

Checks, all exit 0: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo test --package mara --test performance --release --locked` (under `berth --exclusive`; 19 passed, 2 ignored), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`.

## Decisions made

- Request: `query report <name> --id <item>`, with exactly one item, plus the gate's inputs. It explains every row that makes the entry's outcome for that item: an invariant's rows about the item (outcome `failed` with rows, `passed` without), and a gate's rows that `query gate` lists for the item, with its outcome. Result `report` (`name`, `kind`, `item`, `outcome`) and `rows` (`relation`, `role` for gates, `values`, `proof` or `unexplained`). Exit 0 only when every row is explained. Recorded in DES-QUERY-RESULTS.
- The outcome comes from the provenance run's own outputs, read at Soufflé's first reply (readiness probe `explain __mara_ready()`). That keeps the cold count at 1 run, as VER-WORK-COUNTERS requires, and the report does not share the cache of normal evaluations (DES-RULE-EXECUTION).
- A fault before the first reply is reported as the evaluation's fault: outcome `unavailable`, no rows, diagnostics as for a gate. This covers the time-limit case of VER-EVALUATION-TIME-LIMIT, whose fault diagnostic names the limit. After the first reply, rows explained before an abort keep their proofs.
- Proof node format: derived (`relation`, `atom`, `rule` on one line, `children`), fact (`relation`, `values`, `path`, `line`), input (`input`, `values`), negated literal (`absent`). `source_complete` leaves are input leaves named `source_complete`.
- Leaf locations: `title`, `revision` and `field` at the item's line; `folded` at the first item in snapshot order whose title or field value is the text.
- Negated literals with a wildcard are shown as the rule writes them, because Soufflé prints arbitrary values for wildcards (observed: `item("M", "m,r", "m,r", "draft", "m,r", 0)`). Soufflé lists a step's atoms, then its negations, then its constraints; negations are paired with the rule's negated literals by order.
- Aggregates: any step whose rule holds `: {` outside strings, or any leaf of a relation that is no input, cannot be explained. This also catches aggregates that Soufflé shows only as a constraint such as `5 = 5`.
- Bounds: 16 MiB of replies per run; 1,000 steps and 32 KiB per proof (half a page), so that a record fits a page.
- Guide wording: the unbound-server note is shorter ("the project root, else found from where the server started. Pass it for another checkout or worktree"), and the validate bullet drops "before you finish".

## Rejected approaches

- Pairing negated leaves with body literals by position: Soufflé reorders negations after atoms.
- Showing negated atoms as Soufflé prints them in every case: wildcard values are garbage.
- Closing stdout at the reply bound: Soufflé would die of SIGPIPE and the reason would read as an abort.

## Review fixes

The review of `aeb4975` found five points in `crates/mara-engine/src/evaluation/explain.rs`, three of one cause: Soufflé's printed text was classified and flattened as plain text. Each fix has a test that fails without it.

The constraint forms come from Soufflé 2.5's source, the pinned tarball, SHA-256 checked. `ExplainProvenanceImpl.h:740` declares `constraintList` = `=, !=, <, <=, >=, >, match, contains, not_match, not_contains`. Lines 188-200 print the ordered ones (`isOrderedBinaryConstraintOp`) as `<number> <op> <number>` and the rest as `<op>("<sym>", "<sym>")`. `UnitTranslator.cpp:250-272` lists atoms and negations in body order, then the binary constraints. `StringConstant::print` writes a constant raw between quotes. Running Soufflé confirmed it decodes `\"`, `\\`, `\t`, `\n` and `\'` as Mara's lexer does, prints `!contains` as `not_contains`, prints `!(l < 0)` as `l >= 0`, and can abort in `ConcurrentFlyweight::fetch` when a `contains` leaf decodes garbage symbol numbers, which Mara reports as an aborted run.

One reader in the engine (`Vocabulary`) reads what Soufflé prints. It knows the program's decoded string constants (`Program::string_constants`) and relation names. At each quote it takes the one constant that a quote closes there; when none or several do, the text cannot be read and the row is unexplained. Flattening a rule to one line, splitting at top-level separators, finding a step's body literals, finding the arguments of a negated literal, detecting aggregates and classifying leaves all go through it.

1. A negation holding an operator was dropped. Leaves are classified by Soufflé's exact forms: `<int> <op> <int>`, `!<relation>(…)`, `<named constraint>(…)`, and `<relation>(…)`. A step's non-constraint children must equal the rule's atoms and negations in number, no atom or negation may follow a constraint, and every negated literal must have its leaf. Tests: `a_negation_holding_an_operator_stays_in_the_proof` (unit, which also refuses a reply missing the negation) and `programs::explain::values_and_constraints_are_read_as_soufflé_prints_them` (`!denied(m, "a = b")`).
2. Flattening changed constants. Whitespace is collapsed only outside constants, and a rule with an unknown constant is unexplained. Tests: `a_rule_on_one_line_keeps_its_constants` (unit: `"a  b"` and `q"z, "y"`) and the integration test above (owner `a  b`).
3. `not_contains` was taken for a relation. The classifier knows every form of `constraintList`; a named constraint that the program also declares as a relation cannot be told apart, so the row is unexplained. Tests: `every_constraint_form_is_left_out` (unit: all ten forms, a negative number, a declared `not_contains`, an atom after a constraint) and the integration test, whose printed rules hold `>`, `>=`, `<`, `<=`, `!=`, `=` (`strlen(i) = 5`), `l >= 0` from `!(l < 0)`, `contains`, `not_contains` from `!contains`, and the constant `a, "b"`. `match` and `not_match` are refused by the parser and covered by the unit test.
4. A shared MID placed a leaf at the first item. A leaf placed through a MID (`title`, `revision`, `field`, and every holder of a `folded` text) is placed only when exactly one item has that MID; otherwise the row is unexplained, naming the MID. `item` and `edge` leaves keep their own path and line. Tests: `a_leaf_through_a_shared_mid_is_not_placed` (unit: `field`, `title`, `revision`, `folded` unplaced; an `item` leaf at its own line) and `programs::explain::a_leaf_about_a_shared_mid_is_not_placed` (both items unexplained with the MID; `item` leaf placed).
5. An expansion was not checked. The reply to `subproof s(n)` must be a step of `s`, the only part of the tuple the reference gives; otherwise the row is unexplained. Test: `a_subproof_of_another_relation_is_refused` (unit: a reply proving `q("b")`).

Docs: DES-RULE-EXECUTION records the reading with the program's constants, the child order and its checks, the subproof check, the constraint forms, and placement for a shared MID.

### Decisions made in the review round

- A rule or leaf that the program's constants cannot read, a named constraint that is also a declared relation, and a place through a shared MID each leave the row unexplained with the reason; never a guess.
- Expanded subproofs are checked by relation only, since a reference carries no values.

## After mara-jkabqy and mara-tx0fcf

Rebased onto `a5f8ec0`. Beyond a clean replay:

- **mara-jkabqy:** `query report` takes its `--input` files from `Context::cwd`, the client's directory, through the shared `supplied`. The private directory and Soufflé's files go through `caller::tempdir` and `caller::create_new`. `query::REPORT` is in the operation table that the MCP tools and the daemon request use. Test: `daemon::client::the_daemon_resolves_paths_from_the_clients_directory` runs `query report` with `--input note=@note.json` (JSON with debug stats, human form, and a missing file) through the daemon and in direct mode, and requires the same output, diagnostics and exit status, with the proof's input leaf from the client's file.
- **mara-tx0fcf:** the report's performance case is `query report implementation-ready --id <the retired requirement of the last cluster>`, whose failing row `not_accepted` exists at every size (`Corpus::retired_requirement`); DES-PERFORMANCE-HARNESS says so.
- **Reader defect found with the template:** `relation_class` and `risk_dimension` are relations the program states by facts. Soufflé shows their rows as axioms, and the reader took every axiom of a relation that is not an input for an aggregate. So `links-to-accepted`, `supersedes-retired` and `risk-assessed` rows went unexplained. Fixed:
  - `Program::facts` records each fact clause's relation and place, and `Program::fact_values` reads its values when every argument is a literal.
  - Such a leaf is a fact at the program file and line that writes that row.
  - A relation with an aggregate in a rule, a row no fact writes as Soufflé printed it (such as `level(1 + 1)`), or an undeclared relation is still unexplained.
  - Tests: `program::tests::facts_and_string_constants_are_read_as_written`, `evaluation::explain::tests::a_fact_of_the_program_is_placed_at_its_line`, `programs::explain::a_fact_of_the_program_is_a_leaf_at_its_line`.
  - Docs: DES-RULE-EXECUTION, DES-QUERY-RESULTS.
- **Template coverage** (tests in `crates/mara/tests/template/`):
  - Every failing row of the 13 invariants in the invariant cases is explained, `query report` exiting 0.
  - Every item that a gate case finds failed or unavailable gets a report with the same outcome and as many rows, each explained: `implementation-ready`, `verified` and `release-ready`, 51 items over the readiness cases.
  - This covers what mara-zp38s3 asks for VER-TEMPLATE-CASES' "has an explanation" case, without its doc changes.

Checks: all seven local checks at the rebased head, `project validate` with `--no-daemon`.

## After mara-ptq6xc

Rebased onto `52514b9`. `evaluate` keeps mara-ptq6xc's `run_in` split, with this branch's `private_dir`. `spawn_and_wait` both builds the sandbox and takes the console, so the provenance run is sandboxed like every run: it reads the private directory and writes nothing outside `out/`; its pipes are its standard input and output, descriptors 0 and 1. A Soufflé that cannot load a library in the sandbox is reported by `sandbox::library_fault` before its first reply, as `run_in` reports it. Test: `rules::sandbox::tests::the_provenance_run_is_sandboxed_alike` runs the helper's read, write, execute, network and signal modes through `explain` on both ABIs, each stopped, nothing touched outside, and the attempt-free helper as a fault for ending without answering. VER-SOUFFLE-SANDBOX has the row. Every `query report` test, the template's included, passes in the sandbox.
