# Documentation verification

Verification methods state how to assess canonical knowledge. Execution records
identify the reviewed revision and the checks actually performed.

:::mara verification VER-CANONICAL-DOCUMENTATION
:mid: 01M3HD64N99HRMHTVDA2WEW51R
:title: Review canonical contracts and revision-specific evidence
:status: accepted
:method: inspection
:verifies: REQ-CANONICAL-SOURCE

Review every authored product document and its incoming references. Requirements, designs, scenarios, examples and verification methods must explain current behavior without implementation-history context. Keep each durable fact in its owning item and link related methods. Preserve the identities and meaningful relationships of surviving product knowledge. Check examples and named test commands against the actual schema and available suites.

Inspect structured knowledge through Mara. Validate schema and the complete project with the candidate, consume every diagnostic page and inspect nonempty selected intent and verification coverage through all matrix pages. Validation checks machine-readable conformance; a passing result does not establish prose clarity, correct scope or lack of duplication. Review those properties manually.

Keep execution evidence only when it provides a useful, genuine result for current implemented behavior or documentation. Each retained record must identify the actual capture date, checked revision, execution scope and result. Remove retired or superseded evidence after resolving its incoming references; do not retain archives, tombstones or placeholder results in product documentation. Preserve the provenance of retained records and assess them against the verification definition at their subject revision. A changed definition does not turn an earlier execution into a new result. Record fresh inspection evidence for the exact reviewed revision and distinguish documentation checks from application test execution.
:::

:::mara evidence EVD-INTERFACE-PARITY
:mid: 01M3HHTKJTXDNK8FP7221Q4EJJ
:title: CLI and MCP invocation contracts pass with current regression checks
:status: accepted
:result: passed
:captured_at: 2026-09-27T13:45:30Z
:subject_revision: 9b3f4874f2530f30a2084105aadb44bc19550cc5
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-CODE-DISCOVERY
:evidences: VER-ITEM-LIST
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-MID-AND-RECOVERY
:evidences: VER-ITEM-CREATION
:evidences: VER-RELATION-MUTATION
:evidences: VER-ITEM-UPDATE
:evidences: VER-ITEM-DELETION
:evidences: VER-ITEM-MOVEMENT
:evidences: VER-ITEM-RENAME
:evidences: VER-PROJECT-VALIDATION
:evidences: VER-POLICY-VALIDATION
:evidences: VER-TRACE-MATRIX
:evidences: VER-INTERFACE-PARITY
:evidences: VER-CANONICAL-DOCUMENTATION

At subject revision `9b3f4874f2530f30a2084105aadb44bc19550cc5`, `cargo test --locked --all-targets` passed 370 tests across 27 suites with zero failures. Two ignored subprocess helpers were exercised by their passing recovery tests. Formatting and Clippy with all targets/features and warnings denied passed. The complete gate ran against the source and corpus committed at this revision.

Four interface groups check all 30 CLI command help pages, nested parameter guidance for all 21 MCP tools, CLI parse-output selection, and undeclared-argument rejection before side effects. Before the correction, a real MCP backfill request with an undeclared workspace argument inserted a MID; the regression now proves unchanged source. Existing operation suites exercise actual CLI/stdin MCP requests, source preservation, bounded reads, mutations, validation and matrix continuation. The configured rust-analyzer workflow passes in the full suite. Actual generated tool descriptions and representative CLI help were reviewed manually; real repository navigation resolved and read both CLI/MCP implementation symbols through the configured Rust indexer.

Mara inspected all 118 current items across two complete pages and read the complete canonical-documentation method. Manual review of current contracts, standalone skill guidance and the source diff found no dependence on reconstruction history, duplicated current facts or obsolete evidence references. Surviving item identities and relationships were preserved; the interface requirement and its new verification definition state the current invocation contract. Every named verification suite exists.

Candidate schema and project validation completed with zero diagnostics, one page each. Selected intent passed two roots on one page, realization one root on one page, and verification two roots across 1 complete pages. All continuation pages were consumed. Selected exact-revision execution is checked after recording this result; traceability selects evidence and does not execute or authenticate it.

Environment: Linux x86_64, Rust/rust-analyzer 1.97.1; candidate /tmp/mara72-target/debug/mara, debug information and incremental builds disabled. A temporary build quota interrupted attempts before successful execution; only the completed passing run is counted. This result covers interfaces, current corpus inspection and the exercised regression methods. Packaged distribution, manual all-flavour onboarding and release publication are outside this result.
:::
