# Baseline review

Reference: `0b47b42e31a9b1dedb71b1af5658a067c0a739a9` (`0.3.0-alpha.1`).
This review accounts for retained, changed, and removed capabilities. Pending
means the implementation and its distinct verification obligations still need
review; it does not authorize removing behavior. Product contracts live in the
linked capability documents rather than in this inventory.

| Baseline capability | Disposition and review boundary |
|---|---|
| Product intent and scenarios | Retain the two goals and eleven scenarios with their original identities in [product intent](product.mara.md). Add flows for connected editing, interrupted-edit recovery, and code associations from the existing baseline contracts. Remove obsolete release-relative wording from document-context discovery. No behavior change. |
| Canonical source, item identity, document format and schema | Retain source syntax, lossless parsing, item identity and deliberate MID backfill under [format contracts](format.mara.md). Restore all 30 corpus tests, 9 identity/backfill CLI tests and the Markdown-container unit test. Keep distinct Markdown-context, recovery, container/table-span, identity ambiguity and source-preservation obligations. The deterministic repository check is now explicitly read-only and independent of the historical document count. Identity diagnostics use stable codes, severity and source locations, with CLI/MCP parity; backfill also checks existing MIDs and exact preservation of other bytes. Full schema constraints, reference resolution and taxonomy review remain pending. |
| Project discovery, initialization and profiles | Retain initialization, project selection, schema inspection and flavour guidance under [project contracts](project.mara.md). Seventeen baseline tests retain distinct target-selection, transport, template, guidance, inspection and source-preservation obligations in `tests/project_bootstrap.rs`; add CLI/MCP parity for declaration inspection. Fixtures now own their Git/configuration state; guidance assertions use diagnostic codes/severity. Correct obsolete format-2 guidance prose to the implemented schema format 3. Fix the realization check with a requirement-class guard so design and mixed selections evaluate; a new real CLI/MCP regression first reproduced the baseline failure. Engineering policy, classification and execution semantics remain pending separate review. |
| Creation, update, move, rename, deletion and recovery | Pending: mutation contracts and reference preservation; CLI and mutation-reference tests. |
| Item and narrative discovery, search, navigation and bounded reads | Retain Markdown structure, references, source handles and direct navigation under [discovery contracts](discovery.mara.md), with 10 discovery, 5 handle, 13 reference and 3 real CLI/MCP navigation tests. Retain distinct heading-scope, anchor, source-span, process-restart, stale-handle, provenance, namespace and continuation obligations. Fixtures now own Git/configuration and child working directories; named self-host checks are read-only. Broken-reference diagnostics assert stable codes, severity and CLI/MCP parity. Handle and summary meaning now belongs to the structural design; the pending unified-retrieval design must link to it instead of repeating it. Full search ranking, filters, get/list/related continuation and response-bound suites remain pending. |
| Typed relations, aliases, symmetry, inline assertions and external targets | Pending: relation contracts; semantic-edge identity, occurrence inspection, mutation, and terminal external-target tests. |
| Validation, diagnostics and structural graph policies | Pending: independent diagnostics, recovery, reporting filters, graph constraints, and CLI/MCP parity tests. |
| YAML rules and native evaluation | Pending: rule grammar, applicability, bounded paths, evaluation prerequisites, and rule tests. |
| Matrices, parameter binding and pagination | Pending: selected coverage, explanations, revision parameters, bounds, and matrix tests. |
| Code endpoints, comment markers and language adapters | Pending: endpoint resolution and navigation; real code checks and isolated multi-language fixtures. |
| CLI/MCP interfaces and project context | Retain project context and bootstrap transport/parity behavior. Full command help, tool schemas, and remaining operation parity tests are pending. |
| Packaging, installation, migration and releases | Pending: npm packaging and launchers, skill/plugin, migration, CI/release contracts, and real packaged CLI/MCP smoke. |
| Public guidance and historical research/release documents | Pending: README, roadmap, security guidance, generated changelog, and useful historical knowledge. Retained license notices remain unchanged. |

## Runtime and remaining test obligations

Commit `8205108` imported the complete baseline runtime before its capability
reviews. This was not the agreed incremental rebuild. The current bootstrap must
be corrected before advancing another capability; previously recorded results
remain evidence for their exact revisions, not approval of the unreviewed runtime.
The installed baseline executable remains an authoring tool, not the candidate.

The dependency audit establishes a smaller entry boundary: CLI/MCP project
initialization and selection plus schema get/list need configuration, schema
declarations, template publication and transport dispatch. They do not load the
corpus or execute rules. The project type currently imports the code-language
configuration record from the code module; that data dependency does not require
the code scanner.

The earlier bootstrap tests cross that boundary. Schema validation loads YAML
rule declarations; project validation additionally loads source/code, evaluates
rules and graph policies, and paginates diagnostics. Creation and edits require
candidate-corpus validation, link-preservation preflight and transaction recovery.
Source/identity/navigation tests also exercise these operations. Preserving all
those workflows in the active build would retain unreviewed dependencies.

The user approved the narrow boundary on 2026-09-27. The active candidate now
contains five source files: project/schema decoding and template publication,
configuration diagnostics, operation wrappers, CLI dispatch, and stdio MCP.
Only `project init`, `schema get/list`, and their three MCP tools are advertised.
The full implementation remains preserved at `198a3aa` and in the original
baseline. No history is rewritten. Prior reviewed rows above record historical
work; source/identity/navigation/retrieval are not active in this checkpoint.

### Bootstrap correction review

| Area | Decision and reason |
|---|---|
| Project selection and transport | Retain shared OperationContext, nearest-parent discovery and per-call loading. Absolute request paths and bound-server override rejection keep selection unambiguous. Narrow dispatch and help to supported operations; no stub endpoints. |
| Configuration and schema | Retain strict TOML/YAML decoding, formats and declaration sanity checks because inspection must reject malformed configuration. Keep code-language binding metadata as a plain configuration record; omit scanners, rule loading/evaluation and corpus validation. Declaration checks do not establish implemented graph policy. |
| Template publication | Retain embedded sources, complete preflight conflict checks, create-new writes and cleanup limited to files created by the attempt. Preserve the verified realization-template fix; executing that rule awaits its capability. |
| Runtime dependencies | Remove inactive corpus, Markdown, mutation, query, graph, code, rule and trace modules from the build. Retain nine direct runtime dependencies for parsing, matching project patterns and the two transports; tempfile is test-only. Lockfile pruning introduces no dependency version changes. |
| Initialization tests | Retain current/named/explicit targets, ambiguity, existing-content preservation, empty/minimal/engineering files and policy-file conflicts. These independently exercise selection and publication safeguards. |
| Context tests | Retain nearest/explicit selection, outside-project MCP startup, absolute request paths, unbound initialization and bound override rejection. Use schema_get instead of project_validate to observe selected context without importing its engine. |
| Inspection tests | Retain whole-schema, list and named declaration parity, configured schema path, unknown names, and distinct guidance type/content cases. Check malformed guidance through schema_get's operation-error contract; preserve structured validation diagnostic assertions for the later validation capability. |
| Boundary verification | Add CLI help and MCP tools/list checks so the advertised checkpoint matches executable operations. This is temporary rebuild verification, not a permanent reduction of product scope. |
| Deferred tests | Preserve the original bootstrap suite, corpus, discovery, handles, identity, references, navigation, retrieval work and helper source under `tests/pending/*.pending`. They do not compile or enter the source association graph. Restore each with its owning review; no obligation is dismissed. The Markdown-container unit test remains in committed source history. |

`VER-PROJECT-INSPECTION` defines the corrected narrow check. Existing evidence and
`VER-PROJECT-BOOTSTRAP` keep their original revision and broader method. Schema
validation, engineering authoring and realization execution remain unfulfilled
by the reduced candidate even where inspection has a requirement association.
Installed-baseline MCP validates the canonical corpus and selected traceability;
that authoring check is not evidence that the candidate implements validation.

Remaining baseline unit-test modules and integration tests remain pending restoration
with their owning capabilities, including isolated code-adapter fixtures. None
of their distinct obligations has been dismissed as redundant or obsolete.
The old cardinality code association is pending its contract review; new links
identify the reviewed initialization and schema paths. Missing coverage remains
visible in selected-scope matrices.
