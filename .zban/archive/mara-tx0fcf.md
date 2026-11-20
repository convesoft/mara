---
title: Engineering template programs
status: done
type: feature
parent: mara-17tdb3
blocked_by:
  - mara-bx82k0
  - mara-gfb6n6
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T21:10:53Z
---

Goal: the `engineering` template ships the invariants, queries, gates and views that make its vocabulary useful.

Context: docs/template.mara.md (REQ-TEMPLATE-ACCEPTANCE-RULES, DES-TEMPLATE-ACCEPTANCE, DES-TEMPLATE-RELATION-LIFECYCLE, DES-TEMPLATE-RISK-LEVEL); docs/template-readiness.mara.md (REQ-TEMPLATE-GATES-PROVE-READINESS, DES-TEMPLATE-QUERIES, DES-TEMPLATE-INPUTS, DES-TEMPLATE-GATES, DES-TEMPLATE-VIEWS); VER-TEMPLATE-CASES in docs/verification.mara.md.

Design input, on the maintainer's machine: `~/Work/local/workspace/projects/mara-rewrite-design/engineering-template-inputs/README.md` and the Space export beside it. The previous encoding is in `git show d7f0e73:templates/` and `git show d7f0e73:docs/engineering-template.mara.md`; its helper queries, view shims and separate `required_validation` gate were limits of the old engine and are not ported.

- [x] Decide with the maintainer which invariants, queries, gates and views ship; record them as items
- [x] Programs for the acceptance and lifecycle invariants, the queries, the three gates, the three views and the risk matrix, written in the admitted language (DES-PROGRAM-PROFILE) and installed by `project init`; the programs row of VER-PROJECT-INIT passes
- [x] The cases of VER-TEMPLATE-CASES that need no code links pass. Without code links, `verified` and `release-ready` are tested for what they can show: a known failure fails and everything else is unavailable; mara-yhxq5x makes the remaining cases pass
- [x] This repository adopts the invariants, with `requirement-verified` as a warning until the last slice

Decided provisionally by the orchestrator on 2026-11-16 (from mara-b60px3, under the maintainer's rule of that day; listed in mara-46j65e): the four views that the first template leaves out (outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact) are deferred to after 0.4.0; backlog issue mara-7h4z9f.

- [x] DES-TEMPLATE-VIEWS says in one sentence that the other four designed views are not in the first release

## Summary of Changes

- Programs (`crates/mara/templates/engineering/rules/`, admitted language, over `mara.dl` only): `knowledge.dl` (the 13 invariants and the queries `requirement-basis`, `design-basis`, `term-collisions`, `relation-lifecycle`; no closure, so validation computes none), `readiness.dl` (includes `knowledge.dl` and `risk-matrix.dl`; inputs `candidate`, `verification_result`, `validation_context`, `validation_result`; queries `implemented`, `risk-level`, `risk-scope`, `relevant-risks`, `verification-status`, `validation-status`, `result-stamps`; gates `implementation-ready`, `verified`, `release-ready`; views `delivery-readiness`, `requirement-verification`, `risk-treatment`), `risk-matrix.dl` (25 facts). `templates/engineering/config.toml` declares them with an entry each; `templates/config.toml` became `templates/empty/config.toml`.
- `project init --template engineering` writes schema, the three programs, then the configuration (`cli/init.rs`).
- This repository: `.mara/rules/` holds the template's programs (same text, a test compares); `.mara/config.toml` is the template's with `requirement-verified` as a warning. `project validate`: valid, 1 warning (REQ-RUN-WITHOUT-TOOLCHAIN has no verification), 2 program runs, about 0.1 s release and 0.15–0.2 s debug. No item needed a correction.
- Performance corpus: declares the template's programs and entries instead of its stand-in (DES-PERFORMANCE-HARNESS); `results.json` rows carry IDs and current revisions; cases `query execute relevant-risks`, `query view risk-treatment`, `query gate verified` (exit 1, unavailable without code links); `Reads::Checked { runs }` (2 for `project validate`, 1 for `item validate`).
- Tests: `crates/mara/tests/template/` (29): per invariant, accepted items that break it give exactly that diagnostic at those items, the same items as drafts are valid, accepted items that keep it are valid; the readiness cases of VER-TEMPLATE-CASES without code links; `init.rs` programs row of VER-PROJECT-INIT (written, listed, `project validate` compiles with 2 runs, `query list` lists the entries); `mcp/parity.rs` query fixture builds on the template configuration without its entries.
- Docs (worktree binary, `item update`): DES-TEMPLATE-ACCEPTANCE, DES-TEMPLATE-RELATION-LIFECYCLE (invariant names), DES-TEMPLATE-RISK-LEVEL, DES-TEMPLATE-QUERIES, DES-TEMPLATE-INPUTS, DES-TEMPLATE-VIEWS (incl. the sentence on the four deferred views), DES-PROJECT-INIT, VER-PROJECT-INIT, DES-PERFORMANCE-HARNESS, VER-WORK-COUNTERS; README line on `project init`.

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `cargo run --package mara --locked --quiet -- project validate`, `cargo test --package mara --test performance --release --locked` (under `berth --exclusive`), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0.

Not here: the explanation case of VER-TEMPLATE-CASES (mara-zp38s3, blocked by mara-jm5ecb); the code-link cases (mara-hk2arx).

## Decisions made

- One invariant per obligation of a rule, so each message names the fix: `requirement-has-basis`, `requirement-verified`, `design-has-basis`, `risk-assessed`, `risk-threatens`, `verification-has-method`, `verification-has-target`, `validation-has-method`, `validation-has-target`, `term-unique`, `term-aliases`, `links-to-accepted`, `supersedes-retired` (DES-TEMPLATE-ACCEPTANCE, DES-TEMPLATE-RELATION-LIFECYCLE). Each relation has one row per item, so no item gets the same diagnostic twice.
- An empty alias cannot reach a program (malformed row), so `term-aliases` checks only an alias equal to the title, folded. A link to no item is left to `broken-link`.
- Program split and names: `knowledge.dl`, `readiness.dl`, `risk-matrix.dl`; levels `low`, `medium`, `high`; a risk with two values of a field has no level (DES-TEMPLATE-QUERIES, DES-TEMPLATE-RISK-LEVEL).
- Inputs use item IDs; a result other than the four counts as `error`; with more than one `candidate` row, or more than one context for a Validation, no result for it counts; `pending` ranks worse than `unavailable`; the `validation-status` summary row has empty Validation, status and context, `not-validated` without an accepted Validation (DES-TEMPLATE-INPUTS).
- `implemented` reads a `code_link` relation with no rule body that can hold (`:- false`) until mara-hk2arx supplies code links; `source_complete("code")` is never present, so `verified`/`release-ready` are unavailable for every requirement without a failure (DES-TEMPLATE-QUERIES). A Validation can pass `release-ready`.
- Gate relations: subjects `requirement_subject`, `validation_subject`; conditions `not_accepted`, `breaks_rule` (names the invariant relation), `blocking_risk`, `unverified_requirement`, `not_implemented`, `verification_not_passed`, `validation_without_target`, `validation_without_context`, `validation_not_passed`; unavailable `risk_level_unavailable`, `code_unavailable`, `result_unavailable`, `validation_result_unavailable`.
- View cells and rows (DES-TEMPLATE-VIEWS): implementation `direct`/`design`/`missing`/`unavailable`; verification the worst accepted result or `none`; risks `blocking`/`unavailable`/`clear`; a requirement without a Verification has one placeholder row; `risk-treatment` has a row per threatened node.
- The `engineering` configuration is the documented example plus `[rules]` and the entries (DES-PROJECT-INIT); the corpus switched to it as DES-PERFORMANCE-HARNESS foresaw.
- VER-WORK-COUNTERS' validation row said "1 program run per declared program"; DES-RULE-EXECUTION runs only files that an entry names or no other file includes, so an included program (the risk matrix) runs within its includer. The row now says so.

## Rejected approaches

- One invariant per table row: a static message could not say which obligation failed.
- Declaring the code-link relation as an entry input: every request would have to supply it.

## Review fixes

The review of `ddcc69c` found two points.

1. Decided by the orchestrator, no program change: a counting result decides whatever the completeness of its source; completeness matters only without a counting result (`pending` when complete, `unavailable` when not), as DES-TEMPLATE-INPUTS and DES-TEMPLATE-GATES say. So a Validation with a counting `pass` passes `release-ready` from an incomplete source. Test: `template::readiness::a_counting_result_decides_whatever_the_completeness`, with and without `--complete`, for a Verification (`verification-status` gives `pass` and `verified` shows only `code_unavailable`; with no result, `result_unavailable` or `verification_not_passed … pending`) and for a Validation (`release-ready` passed; with no result, unavailable or failed).
2. `risk-treatment` gave only `accepted` and no reference to the accepting Decision. Its column `accepted_by` now holds the MID of each accepted Decision that accepts the risk, a row for each, as `requirement-verification` names its Verification; empty when none does, and a draft Decision is not named (DES-TEMPLATE-VIEWS; `readiness.dl`, this repository's copy identical; the view's description in both configurations). Test: `template::readiness::risk_treatment_names_each_accepting_decision` (no accepted Decision with a draft one: empty, blocking, gate failed; one: `ADR-A`, not blocking, gate passed; two: a row each).

### Decisions made in the review round

- Decided by the orchestrator: a counting result decides whatever the completeness of its source (point 1).
- `risk-treatment` names accepting Decisions with a row per Decision rather than one cell: Soufflé cannot join symbols into one cell, and a row per Decision keeps each cell one item, as the other views do.
