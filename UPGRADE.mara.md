# Repository rebuild handoff

## Agreed scope and starting point

The user wants to rebuild the entire repository incrementally: documentation,
implementation, tests, configuration, and delivery tooling. Review existing
behavior, simplify where justified, remove obsolete material and redundant tests,
and establish meaningful code links while each capability is understood. Reusing
unchanged code is a valid result. There is no target test count or requirement to
rewrite sound code. This thread prepared the handoff; substantive rebuilding
starts in a new thread.

- Work only on the user-selected branch `feature/mara-upgrade`, in the sibling
  directory ending `mara-72-align-the-self-hosted-corpus-with-03-traceability--feature%2Fmara-upgrade`.
- Reference baseline: commit `0b47b42e31a9b1dedb71b1af5658a067c0a739a9`, version
  `0.3.0-alpha.1`. The sibling MARA-72 worktree contains that baseline; treat it
  as a reference. Use the immutable commit if that branch later moves.
- The engineering template and intent-to-production workflow are agreed inputs.
  Read `docs/engineering-template.mara.md`, `docs/engineering-workflow.mara.md`,
  and the `templates/engineering-*.yaml` sources before choosing vocabulary.
- MARA-72 owns the preceding template/corpus work; MARA-74 parameters are already
  integrated. The expanded repository rebuild scope is recorded here. The Linear
  issue still describes the earlier corpus scope; reconcile delivery tracking
  with this scope when continuing that issue. No issue is complete merely because
  this preparation or a matrix passes.
- Preserve supported behavior unless a concrete change is explicitly settled and
  reflected in canonical requirements/designs. Ask one plain-text question and
  wait when compatibility, public behavior, data safety, or accepted scope needs
  the user's decision. Continue independently on settled work.

## Reference access and installed tools

The installed `mara` is `/home/araketski/.local/bin/mara`. CLI and Codex MCP use
the same binary snapshot built from the baseline above. `mara --version` reports
`0.3.0-alpha.1`; the matching skill was installed from the MARA-72 checkout with
`npx skills`. Both the binary and skill are installed copies. Editing this new
worktree does not automatically update either one.

Pass this worktree's absolute path as `project` to every Mara MCP call. The server
is unbound, and its execution directory may belong to the previous worktree.
For CLI commands, run from this worktree or pass `--project` explicitly.

The baseline's self-hosting `.mara/schema.yaml` still uses `code_implements` and
`code_verifies`; it has not been migrated to the engineering profile's lifecycle,
relations, or enabled policies. Its format-3 project configuration includes four
language packs. A valid baseline corpus does not demonstrate the new profile's
coverage. Migrate configuration, items, rules, and assertions coherently; do not
drop language configuration by copying a format-2 template project file over it.

Read old files without restoring them into the new corpus:

```sh
git show 0b47b42:docs/alpha.mara.md
git show 0b47b42:src/lib.rs
```

Installation provenance is stored beside the installed binary in
`~/.local/share/mara/checkouts/0b47b42e31a9b1dedb71b1af5658a067c0a739a9/source.json`.
The previous published installation and configuration were backed up under
`~/.local/state/mara/before-checkout-20260926T205958Z`.

## Initial capability inventory

These are baseline entry points, not decisions to preserve every implementation
detail or proposed new module boundaries. Every row awaits substantive review.

| Capability | Contracts to read at the baseline | Implementation and verification entry points |
|---|---|---|
| Intent, identity, source format and schema | `docs/alpha.mara.md`: GOAL-UNIFIED-PROJECT-KNOWLEDGE, GOAL-BOUNDED-AGENT-CONTEXT, REQ-CANONICAL-SOURCE, REQ-DURABLE-ITEM-IDENTITY; `docs/format.mara.md`, `docs/taxonomy.mara.md` | `src/lib.rs`, `src/corpus.rs`, `src/corpus/`; `tests/corpus.rs`, `tests/references.rs` |
| Project discovery, initialization and profiles | `docs/alpha.mara.md`: REQ-PROJECT-INITIALIZATION, DES-PROJECT-CONFIGURATION; `docs/guided-authoring.mara.md`, `docs/engineering-template.mara.md` | `src/lib.rs`, `templates/`; engineering/init cases in `tests/cli.rs` |
| Creation, update, move, rename, deletion and recovery | `docs/alpha.mara.md`: REQ-ITEM-CREATION; `docs/editing.mara.md` | `src/mutation.rs`, `src/mutation/`, `src/operations.rs`; mutation cases in `tests/cli.rs`, `tests/mutation_references.rs` |
| Item and narrative discovery, search, navigation and bounded reads | `docs/discovery.mara.md`, `docs/retrieval.mara.md`, `docs/guided-authoring.mara.md` | `src/discovery.rs`, `src/discovery/`, `src/query.rs`, `src/query/`; `tests/discovery.rs`, `tests/discovery_handles.rs`, `tests/query.rs` |
| Typed relations, aliases, symmetry, inline assertions and external targets | `docs/relations.mara.md`, `docs/traceability.mara.md` | `src/relations.rs`, `src/external.rs`, corpus/reference handling; relation cases in `tests/cli.rs` |
| Validation, diagnostics and structural graph policies | `docs/rules.mara.md`: DES-TRACE-DIAGNOSTIC-INTERFACE, DES-TRACE-GRAPH-CONSTRAINTS | `src/diagnostics.rs`, `src/graph_constraints.rs`, `src/operations/validation.rs`; validation cases in `tests/cli.rs` |
| YAML rules and native evaluation | `docs/rules.mara.md`: DES-TRACE-RULE-GRAMMAR | `src/rules.rs`, `src/rules/`; rule cases in `tests/cli.rs` |
| Matrices, parameter binding and pagination | `docs/rules.mara.md`: DES-TRACE-VIEW-INTERFACES; `docs/traceability.mara.md`: REQ-TRACE-MATRIX | `src/trace.rs`, `src/rules.rs`; trace matrix cases in `tests/cli.rs` |
| Code endpoints, comment markers and language adapters | `docs/code-traceability.mara.md`: REQ-CODE-TRACEABILITY, DES-CODE-TRACEABILITY | `src/code.rs`, `.mara/code/`; unit and four-language integration tests |
| CLI/MCP interfaces and project context | `docs/alpha.mara.md`: REQ-SURFACE-PARITY, DES-OPERATION-PROJECT-CONTEXT; interface contracts in feature documents | `src/main.rs`, `src/mcp.rs`, `src/operations.rs`; transport and parity cases in `tests/cli.rs` |
| Packaging, installation, migration and releases | `docs/distribution.mara.md`, `docs/migration-0.2.mara.md`, `docs/migration-0.3.mara.md`, `docs/release-0.1.mara.md` | `scripts/`, `npm/`, `skills/mara/`, plugin metadata, `.github/`; packaged CLI/MCP smoke workflow |

Also review `README.md`, `ROADMAP.md`, `SECURITY.md`, `AGENTS.md`, and historical
release/research documents for current usefulness. Preserve license notices and
attribution when retaining third-party assets.

For each capability, record its retained obligations, intentional changes or
removals, implementing code, distinct verification obligations, and unresolved
questions. Use canonical Mara items for durable meaning and the delivery tracker
for work status. Avoid a second permanent specification in this handoff.

## Starting the rebuild

1. Read `AGENTS.md` and this handoff; confirm branch, clean state, baseline access,
   installed version, and explicit MCP project selection. Recover changed local
   work before any reset. Keep this handoff accessible through Git history.
2. Prepare the exact retained-file and removal list. Preserve the worktree's
   `.git` control file and shared Git metadata. The proposed bootstrap retains
   this handoff, licenses, reviewed agent instructions, ignore rules, pinned Rust
   toolchain and Cargo setup, the engineering profile, and language assets needed
   for the first slice. Account for untracked files separately. Use bounded,
   tracked deletions rather than clearing the directory indiscriminately.
3. Establish a minimal buildable bootstrap and canonical index. Revisit product
   intent, actors where useful, and scenarios. Carry existing IDs/MIDs forward
   when their meaning survives; preserve identity through structured moves and
   renames. Splits, merges, replacements, and removals need explicit treatment
   of existing references. Do not regenerate old MIDs through recreation.
4. Start with an end-to-end slice: initialize an engineering project, author
   meaningful knowledge, connect and retrieve it, and validate it through CLI
   and MCP. Reintroduce further capabilities in dependency order. Draft status
   permits incomplete knowledge while requirements and checks take shape.
5. For each slice, review the contract, implementation, and tests together;
   add meaningful code associations; execute checks; assess traceability; and
   commit a working checkpoint. Keep unimplemented baseline capabilities visible
   as pending work. A scaffold passing its small suite is not completion of the
   repository rebuild.

Changes to dependencies, abstractions, module layout, and test infrastructure
must solve an observed need in the selected slice. Use existing standard tools.
Do not add scaffolding or verification infrastructure merely to increase coverage.

## Code links and evidence

Use the engineering profile's vocabulary once it is installed in this project's
schema. Add links while reviewing the code and its obligation:

- `implements`: code implementing a requirement, durable design, or independently
  described verification method.
- `checks`: test/check code directly checking a requirement or design.
- `verifies` / `validates`: an independently useful verification definition links
  to its requirement/design or goal/scenario.
- `evidences`: an actual recorded execution result links to its verification.
- `realizes`: an independently useful artifact realizes a requirement/design.

For example, after resolving the real requirement and symbol, author a comment
immediately before the relevant declaration:

```rust
// @mara implements REQ-EXAMPLE
fn implementation() { /* ... */ }

// @mara checks REQ-EXAMPLE
#[test]
fn observable_behavior() { /* ... */ }
```

These are syntax examples, not items or evidence to create. Item-authored inverse
links such as `implemented_by` and `checked_by` are also supported. Prefer the
narrowest stable endpoint that communicates ownership; a file endpoint is useful
when the whole file is the implementation unit. Inspect actual symbol selectors
with Mara. Do not duplicate assertions on both sides without a reason, link every
helper mechanically, or create placeholder items to complete chains.

Mara validates source integrity, graph constraints, declared obligations, and
selected coverage. It does not execute tests or prove code correctness, evidence
authenticity, absence of bugs, or production readiness. Run real checks and review
their results alongside the matrices. Execution checks require accepted passing
evidence for the actual tested revision; code links and historical passing runs
alone do not establish that result. Record dirty-build differences and environment
where relevant. Never label an untested revision as passing.

## Test review

Baseline discovery enumerated 336 Rust tests: library 26, binary 1, CLI integration
238, corpus 30, discovery 10, discovery handles 5, mutation references 10, query 3,
and references 13. This is an inventory, not a new full-suite execution. The prior
full run passed 334 with 2 ignored before the version-only alpha.1 bump.
`tests/cli.rs` is the largest review entry point; size alone is not a defect.

For each test or related group, identify the observable obligation, failure it
detects, verification level, real boundaries exercised, and any overlapping test.
Choose keep, consolidate, replace, or remove with a concrete reason. Preserve
the distinct obligation before deleting redundant coverage. Avoid quotas.

Shared code paths are not sufficient evidence of duplication. Unit tests can
isolate important behavior; CLI and MCP tests can catch separate serialization,
project-context, and transport failures. Keep representative real end-to-end
workflows and consequential failure cases, especially data preservation. New bugs
need a reproduction and targeted regression test. Correctly passing mocks do not
establish that the real workflow operates.

## Verification and completion

For every coherent slice, require current canonical knowledge, meaningful code
links, relevant passing tests, and complete schema/project validation. Matrix root
selection must include the intended items; an empty selection proves no coverage.
Follow pagination and inspect failed/unavailable states and nonqualifying targets.
Assess intent, realization, verification, validation, and execution where the
selected slice genuinely requires them; document why a check is inapplicable.

After the profile migration, these request-local examples apply to real selected
IDs, replacing the placeholders below:

```sh
mara trace matrix --id REQ-EXAMPLE --check-file .mara/engineering-checks.yaml --shape urn:mara:rule:realization
mara trace matrix --id VER-EXAMPLE --check-file .mara/engineering-execution.yaml --shape urn:mara:rule:execution --param subject_revision=TESTED_REVISION
```

For code changes, retain the existing CI gates unless an explicit review changes
them: `cargo fmt --all -- --check`, strict Clippy, `cargo test --locked --all-targets`,
and project validation. Use targeted checks during a slice; run the full required
gates at coherent integration points. Packaging changes also need the real
`scripts/smoke-npm.sh` workflow. Match diagnostics by code/severity, not prose.

This machine's percent-encoded worktree path previously caused linker problems.
Use a target directory outside it; `/tmp/mara72-target` already holds dependency
builds. `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`
avoids the debug-artifact disk pressure observed during preparation. The package
smoke can use `npm_config_cache=/tmp/mara72-npm-cache npm_config_offline=true`.
These are local execution details, not changes to the product contract.

If executable contracts or the skill change, rebuild and align the local CLI,
MCP, and skill again before using them to evaluate those new contracts. Tests of
the candidate must invoke its freshly built binary. The baseline installed tool
remains useful for bootstrapping and reading compatible knowledge.

The rebuild is complete when every baseline capability has a reviewed disposition,
retained behavior works end to end, intended changes are documented, required
checks have actual evidence, and the corpus exemplifies the engineering workflow.
Commit bounded verified changes. Publication, merging, release tags, and marking
issues Done remain separate delivery actions governed by `AGENTS.md`.
