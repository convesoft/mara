---
title: Write the Verification items for the alpha.3 requirements
status: done
type: task
priority: high
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-16T11:06:45Z
---

Goal: every requirement that the alpha.3 slice delivers has an accepted Verification item: the requirements that the issues of mara-17tdb3 name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [x] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [x] One Verification item for each, or one shared by requirements that a single check covers
- [x] The test items in those issues point at the Verification items instead of restating them

Started on 2026-11-16 before the alpha.2 epic closed, by the orchestrator's decision under the maintainer's rule of that day: this issue writes documents only and depends on nothing the last two alpha.2 issues build, so starting it now shortens the path to alpha.3. The alpha.3 epic stays blocked by the alpha.2 epic.

## Summary of Changes

The slice delivers these requirements. Each now has a Verification item:

| Requirement | Where the slice names it | Verification |
| --- | --- | --- |
| REQ-PROJECT-AUTHORED-LOGIC | mara-bx82k0; DES-RULE-CONSUMERS, DES-GATE-OUTCOME, DES-FACT-BRIDGE (mara-y4asgm) | VER-PROGRAM-CONSUMERS, VER-FACT-ROWS (new) |
| REQ-EXTERNAL-RECORDS-AS-SUPPLIED | mara-bx82k0; DES-REQUEST-INPUTS (mara-y4asgm) | VER-REQUEST-INPUTS (new) |
| REQ-PROGRAMS-ONLY-COMPUTE | mara-wrbjte, mara-bx82k0; DES-PROGRAM-PROFILE | VER-PROGRAM-REACH-CASES (every row of EVD-SOUFFLE-PROGRAM-REACH), VER-ADMITTED-LANGUAGE (new) |
| REQ-PROGRAM-FAULTS-REPORTED | mara-bx82k0; DES-RULE-EXECUTION | VER-PROGRAM-FAULTS (new), with the output-size and memory limits |
| REQ-EVALUATION-TIME-LIMIT (Draft) | mara-bx82k0 | VER-EVALUATION-TIME-LIMIT (new, Draft with its requirement) |
| REQ-EXPLAIN-OUTCOMES | mara-bx82k0; DES-RULE-EXECUTION | VER-EXPLANATIONS (new) |
| REQ-TEMPLATE-ACCEPTANCE-RULES, REQ-TEMPLATE-GATES-PROVE-READINESS | mara-tx0fcf | VER-TEMPLATE-CASES, with rows for every acceptance and lifecycle rule, the 25 risk cells, query statuses and the three views; the design-only case marked alpha.5 |
| REQ-PROJECT-INIT | DES-TEMPLATE-VIEWS (mara-tx0fcf) | VER-PROJECT-INIT, programs row extended |
| REQ-CLI-MCP-PARITY | mara-bx82k0 | VER-CLI-MCP-PARITY, `query` row unchanged |
| REQ-WORK-FOLLOWS-CHANGE, REQ-NEAR-LINEAR-SCALING | mara-bx82k0 | VER-WORK-COUNTERS (alpha.3 cold rows), VER-SCALING; DES-PERFORMANCE-HARNESS extended |

Not in this slice: REQ-RUN-WITHOUT-TOOLCHAIN (DES-SOUFFLE-BUILD; npm in 0.4.0, mara-haczya), REQ-SAME-RESULT-ON-EVERY-HOST for Soufflé in the browser (alpha.6).

- New pages docs/verification-rules.mara.md and docs/verification-programs.mara.md, listed in docs/index.mara.md.
- DES-RULE-EXECUTION: the private directory is a new temporary directory removed when the evaluation ends; how faults are reported on each consumer; a supplied-input leaf of an explanation names the input. DES-REQUEST-INPUTS: an input the consumer does not list is refused.
- DES-PERFORMANCE-HARNESS: the generated configuration declares programs from alpha.3; gate and report cases.
- Test items now name the Verification items in mara-wrbjte, mara-y4asgm, mara-bx82k0 and mara-tx0fcf; mara-xsm8w6 gets the browser rows.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate` (valid: 0 errors, 0 warnings), `cargo test --package mara --test performance --release --locked` (14 passed), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0. The worktree's binary showed no Verification for the six rules requirements before the change and the items above after it (`relation list <REQ> --relation verified_by`). A script found 209 distinct MIDs and blank lines on both sides of every `---` on the new pages; every page is within 200 lines.

## Decisions made

- Native only in alpha.3: the rows that compare the browser's Soufflé start with mara-xsm8w6 (alpha.6), which builds it, as VER-HOST-COMPARISON already schedules the browser. mara-bx82k0 and mara-y4asgm asked for WASM tests, but no issue of this slice builds Soufflé with Emscripten.
- VER-EVALUATION-TIME-LIMIT is Draft: its requirement is Draft (provisional number), and the strict `Verifies` lifecycle forbids an accepted Verification of a draft requirement. The two never-ending programs of EVD-SOUFFLE-PROGRAM-REACH pass when stopped by the time or the memory limit, whichever comes first.
- Fault reporting (DES-RULE-EXECUTION): an error diagnostic at the project path of the program file (never the private copy) and line, message ending with the source line and a caret; a fault without a line, or from a limit, at the configuration line that declares the program and naming the limit; `project validate` invalid, `query execute` and `query view` fail, a gate and every item it names unavailable. Gate exit statuses are left to mara-bx82k0.
- The private directory is removed when the evaluation ends, so the reach cases can require that no file under the test's temporary directory, `TMPDIR` included, changed.
- An input that the consumer does not list is refused, as an undeclared argument is (DES-REQUEST-INPUTS).
- A leaf from a supplied input names the input instead of a document location (DES-RULE-EXECUTION).
- Cold counters: `project validate` runs each declared program once; `query execute`, `view` and `gate` once; `query report` once in provenance mode; `query list` and refused requests never. In CLI direct mode subprocess spawns equal program runs, which also shows that Mara runs no tests.
- The harness's gate case supplies a complete `verification_result` with one `pass` row per `Verifies` edge, without matching revisions; the report case explains a failing row of that gate. A program written in the test stands in until mara-tx0fcf.
- The libfunctors cases build the library with `cc` in the test: a C compiler is present wherever Rust links. `mara` itself runs without one on `PATH`.
- The review probes and the rewritten trial programs are copied into the repository as fixtures, with the rows the probes recorded natively, which serve as expected values for the admitted arithmetic and text functions.
- Decided by the orchestrator: the explanation checks that no admitted program is known to reach (proof for another row, a reply that is not JSON, an aborted provenance run) are verified by inspection when mara-bx82k0 is reviewed, as VER-EXPLANATIONS says. Rests on ADR-DEBUG-PAUSE-POINT, which keeps test-only hooks to the writer.
- Decided by the maintainer (review): REQ-EXPLAIN-OUTCOMES now allows a leaf that names the supplied input that brought its fact, instead of a document path and line. Reason: the `engineering` gates consume supplied results (REQ-EXTERNAL-RECORDS-AS-SUPPLIED), so under the old wording the outcomes users most need explained could never be explained, against NEED-CONFORMANCE-FEEDBACK. Question: "Provisional decision 3 of 3, nothing waits on your answer. REQ-EXPLAIN-OUTCOMES (Accepted) says an explanation's leaves are input facts with document path and line. The template's gates also consume records supplied from outside, such as test results, which have no document location. Under the current wording such an outcome could never be explained. I amended the requirement so that a leaf from a supplied input names that input. Confirm?" Chosen option: "Leaf names the input (Recommended) — Keep my decision: an explanation may end in a supplied record and names the input it came from. The outcomes users most need explained, the release gates, can then be explained." Not chosen: "Keep the old wording — An outcome whose explanation would end in a supplied record is reported as "cannot explain". The requirement stays as written; gates that use test results get no explanation." I decided what such a leaf shows: the input's name and the row's values as supplied, not a position in the request, since Soufflé keeps one copy of duplicate rows (DES-RULE-EXECUTION, VER-EXPLANATIONS).
- REQ-RUN-WITHOUT-TOOLCHAIN has no Verification in this slice, confirmed by the orchestrator: it is about `npx @convesoft/mara@<version>`, which 0.4.0 delivers (mara-a4m588); alpha.3 builds only the Soufflé it needs. mara-haczya now names it and requires rule evaluation with the packaged Soufflé.
- VER-FACT-ROWS copies `edge`, `folded`, `source_complete` and supplied inputs into relations headed by an item's MID (edge by its source; folded joined to the title or field value it folds; the others joined to every item), as DES-RULE-CONSUMERS requires; the long, memory and output-size programs of docs/verification-programs.mara.md give one row per item for the same reason.

## Rejected approaches

- Pinning the exact values of `to_number` of hexadecimal text in the docs: not established from the evidence; the probes' recorded rows are the reference instead.
- A time-limit case with a program that grows without bound: it may reach the memory limit first. The count over a four-fold product needs hours with little memory.

## Review

One review of `3996c1d` reported five findings; the orchestrator decided each:

1. VER-TEMPLATE-CASES lacked stale validation results: added a row for a validation result of another context or an older revision of the validation, need or goal (`pending` with a complete input, `unavailable` with an incomplete one, no `release-ready` pass), and stated the same statuses for the verification row.
2. VER-TEMPLATE-CASES lacked the worst-result rule: added `pass` with `fail`, `error` or `inconclusive` for one verification and requirement and for one validation and need; the worse status, and `verified` and `release-ready` fail.
3. Supplied-input leaves went beyond REQ-EXPLAIN-OUTCOMES: the requirement now allows such a leaf (decided by the maintainer, see Decisions made); DES-RULE-EXECUTION and VER-EXPLANATIONS say the leaf holds the input's name and the row's values.
4. VER-FACT-ROWS broke the consumer contract: every consumed relation now starts with an item's MID, with the projections listed in its setup; the setup of VER-PROGRAM-CONSUMERS states the rule for the other items.
5. REQ-RUN-WITHOUT-TOOLCHAIN: exclusion kept; mara-haczya names it.

Checks after the fix, on a clean tree: `cargo fmt --all --check`, clippy, `cargo test --workspace --locked`, `project validate` and `zban check` exit 0.
