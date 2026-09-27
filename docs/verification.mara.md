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

:::mara evidence EVD-ENGINEERING-WORKFLOW
:mid: 01M3HJY6WKSTBWARA1YHMXSGR3
:title: Engineering authoring and current regression workflows pass
:status: accepted
:result: passed
:captured_at: 2026-09-27T14:04:57Z
:subject_revision: ced0de4c238637a18756f8a8ee880d7263cf95dd
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
:evidences: VER-PROJECT-BOOTSTRAP

At subject revision `ced0de4c238637a18756f8a8ee880d7263cf95dd`, cargo test --locked --all-targets passed 372 tests across 27 suites with zero failures. Two ignored subprocess helpers were exercised by their passing recovery tests. Formatting and all-target/all-feature Clippy with warnings denied passed. A focused run passed all 19 bootstrap and 16 matrix groups. The final full gate ran against the source and corpus committed at this revision.

The engineering authoring workflow created all eleven supported flavours through both real CLI and stdio MCP processes in disposable projects. It added typed relationships, compared both traversal directions across transports, refused invalid source/target flavours without changing source, and validated the resulting knowledge. A separate workflow selected an accepted design alone and a mixed requirement/design set: both completely failed realization before a direct file-code implementation was authored on the design, then passed with exact counts and equivalent CLI/MCP results. The lifecycle matrix workflow distinguishes acceptance, implementation, check definitions, failed results and passing evidence for an exact revision.

The complete suite also exercised invocation guidance/unknown-argument preservation, parsing, identity, discovery, bounded retrieval, relationships, source edits/recovery, validation and trace matrices. Its genuine configured rust-analyzer workflow passed. Repository navigation through the real configured Rust indexer resolved and read both engineering verification functions completely; synthetic project fixtures do not substitute for that indexer check.

Mara inspected all 124 source items over two complete pages and read the canonical-documentation method. Manual review assessed the engineering profile and workflow for clear current behavior, actionable examples and canonical ownership of vocabulary/policy facts. A source comparison confirmed surviving IDs/MIDs and unaffected product items are unchanged. The distinct packaged-distribution record retains its actual revision, capture date and scope; no earlier execution is relabeled, and no retired evidence, archives or tombstones remain.

Candidate schema and project validation completed with zero diagnostics, one page each. Selected intent passed two roots on one page; realization two roots on one page; verification three roots across 2 pages; scenario validation two roots on one page. Every continuation page was consumed. Exact-revision execution is checked after recording the result; matrix coverage does not execute tests or authenticate evidence.

Environment: Linux x86_64, Rust/rust-analyzer 1.97.1, candidate /tmp/mara72-target/debug/mara, debug information and incremental builds disabled. This record covers the engineering authoring workflow, current corpus inspection and the exercised regression methods. It does not claim a new packaged-distribution run or release publication.
:::
