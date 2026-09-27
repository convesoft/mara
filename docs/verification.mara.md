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

:::mara evidence EVD-SCHEMA-EVOLUTION
:mid: 01M3HKYD6MHT0EF07S9AK40D2D
:title: Schema evolution, regression and packaged host workflows pass
:status: accepted
:result: passed
:captured_at: 2026-09-27T14:22:32Z
:subject_revision: bd45115d610a1ec0b242d9628d7b10525626c7d6
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
:evidences: VER-NPM-DISTRIBUTION
:evidences: VER-SCHEMA-EVOLUTION
:evidences: VER-SOURCE-AND-IDENTITY

At subject revision `bd45115d610a1ec0b242d9628d7b10525626c7d6`, cargo test --locked --all-targets passed 374 tests across 28 suites with zero failures. Two ignored subprocess helpers were exercised by their passing recovery tests. Formatting and all-target/all-feature Clippy with warnings denied passed. Focused schema execution passed both alias workflows and all 20 schema-definition groups; the final full run includes the completed CLI/MCP direction checks.

A customized schema gained an inverse alias, retained three occurrences of one canonical edge, rejected an unrewritten alias rename, then validated after its authored spellings were updated. CLI/MCP inspection agreed, and custom enum fields, MIDs, unrelated prose, verification source and project settings remained intact. The same-flavour removal workflow demonstrated that a naive same-item rewrite can validate with reversed direction. Canonical-source reauthoring, inverse metadata removal and inline demotion retained the intended edge and rejected its reverse through both transports, with exact intended source bytes. These are manual file edits followed by real Mara inspection and validation.

The complete suite also exercised parsing and source/identity recovery, all-flavour engineering authoring, interface invocation, discovery/retrieval, code indexing, relation inspection/mutation, item edits, validation and matrices. The genuine configured rust-analyzer workflow passed. Repository navigation with the real Rust indexer resolved both schema verification functions and read their complete declarations.

scripts/smoke-npm.sh passed against the real candidate and matching updated skill on Linux x86_64, glibc 2.44, Node.js 26.7.0 and npm 11.19.0. Local tarballs installed offline with --ignore-scripts. Actual main-package files were exactly bin/mara.cjs, skills/mara/SKILL.md, README.md, LICENSE-MIT, LICENSE-APACHE and package.json; native files were bin/mara, README.md, both licenses and package.json. Workspace version, four exact native optional dependencies, absent lifecycle scripts and installed skill bytes were checked. The Node-only PATH workflow ran installed CLI/MCP initialization, templates, guidance, authoring, navigation/backlinks, every continuation page, bounded Unicode reconstruction, stale-result rejection and custom-schema repair. All four native manifests also passed metadata inspection with non-executable fixture payloads; native execution here covers Linux x64 only.

Mara inspected all 127 source items across two complete pages and read the documentation verification method. Manual review checked the current source-edit guide, its rationale and verification, and the skill's reference to the canonical procedure. Surviving product items were compared exactly; identities and unaffected bodies/relationships are unchanged. No retired or superseded execution records, archives or tombstones remain in the source corpus. Candidate schema/project validation completed without diagnostics, one page each. Intent passed two selected roots on one page; verification passed three roots on one page. All continuation pages were consumed. Exact-revision execution is checked after recording this result.

Environment for Rust checks: Rust/rust-analyzer 1.97.1, candidate /tmp/mara72-target/debug/mara, debug information and incremental builds disabled. This record covers the performed regression, schema-editing, documentation and packaged host workflows. It does not establish execution on other native targets, a published release or an automated schema-transformation command.
:::
