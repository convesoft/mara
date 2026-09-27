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

:::mara evidence EVD-REPOSITORY-WORKFLOWS
:mid: 01M3HP1B4DM0C872HMBWSKKB0Q
:title: Repository and packaged host workflows pass
:status: accepted
:result: passed
:captured_at: 2026-09-27T14:59:05Z
:subject_revision: d41e0416833e610b9ed35b9068604774ab181cfa
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-CANONICAL-DOCUMENTATION
:evidences: VER-CODE-DISCOVERY
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-INTERFACE-PARITY
:evidences: VER-ITEM-CREATION
:evidences: VER-ITEM-DELETION
:evidences: VER-ITEM-LIST
:evidences: VER-ITEM-MOVEMENT
:evidences: VER-ITEM-RENAME
:evidences: VER-ITEM-UPDATE
:evidences: VER-MID-AND-RECOVERY
:evidences: VER-NPM-DISTRIBUTION
:evidences: VER-POLICY-VALIDATION
:evidences: VER-PROJECT-BOOTSTRAP
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-PROJECT-VALIDATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-RELATION-MUTATION
:evidences: VER-RELEASE-PREPARATION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-SCHEMA-EVOLUTION
:evidences: VER-SOURCE-AND-IDENTITY
:evidences: VER-TRACE-MATRIX
:evidences: VER-UNIFIED-SEARCH

At subject revision `d41e0416833e610b9ed35b9068604774ab181cfa`, cargo test --locked --all-targets passed 382 tests across 29 suites with zero failures. Two ignored subprocess helpers were exercised by their passing recovery tests. cargo fmt --all -- --check and cargo clippy --locked --all-targets --all-features -- -D warnings passed. The source tree remained unchanged during these checks.

The suite exercised initialization and all-flavour engineering authoring, strict schema/configuration definitions, parsing and identity, bounded discovery/retrieval/navigation, canonical relations and source-safe edits/recovery, conditional policy and matrices, CLI/MCP invocation contracts and manual schema edits. Real configured rust-analyzer indexing passed alongside deterministic fixture cases. The full run includes 52 initialization, navigation and authoring-document tests, including Unicode setext/tab/EOF source spans, nested Markdown edits, graph reload after an actual update, stable MID resolution after rename/move, current-directory content patterns and incoming code/item/symmetric ordering through real transports.

scripts/smoke-npm.sh passed with the candidate on Linux x86_64, glibc 2.44, Node.js 26.7.0 and npm 11.19.0. Actual main-package contents were exactly bin/mara.cjs, skills/mara/SKILL.md, README.md, both license texts and package.json; native contents were bin/mara, README.md, both license texts and package.json. Workspace version, four exact native optional dependencies and absence of lifecycle scripts were checked. Local tarballs installed offline with --ignore-scripts into isolated storage. With only Node on PATH, the installed CLI and stdio MCP completed templates/guidance, authoring, narrative/section/document navigation, backlinks, full pagination, Unicode reconstruction, stale-result rejection and custom-schema repair. Installed skill bytes matched source. All four native manifests passed generation/metadata inspection with non-executable fixture payloads; native execution here covers Linux x64 only.

The real candidate MCP server initialized and returned complete repository validation and a configuration-item read identical to CLI domain results. The configured Mara MCP connection independently returned the current design vocabulary and complete valid schema with no diagnostics. Mara inspected all 135 source items across two pages and read both configuration designs and the navigation verification definition completely. Manual review checked current behavior, item identities, meaningful relationships, supported installation paths and the release guide. Configuration and vocabulary rules were compared with their implementation; source identity/integrity checks passed. Canonical documentation contains current specifications and scoped execution evidence.

Candidate schema/project validation completed with zero diagnostics, one page each. Request-local coverage passed every selected root: intent: 42 roots, 11 pages; verification: 79 roots, 24 pages; realization: 6 roots, 2 pages. All continuation pages were consumed. Execution coverage is checked at this subject revision after recording this result; a matrix selects the recorded result and does not execute tests.

Release-source inspection confirmed captured-revision propagation, four host jobs, source/artifact gates, protected publish-job dependencies/permissions, tag-target and package-digest refusal, native visibility before dispatcher publication and public smoke before GitHub publication. Workflow YAML parsed, Bash syntax passed for all 23 CI/release run steps and the smoke script, and both JavaScript entry points passed node --check. Public guidance, provisional roadmap, security-reporting instructions and license references were reviewed. These are local source/host checks, not evidence of remote protection settings, approval, other-host execution or publication.

Rust and rust-analyzer were 1.97.1. Rust checks used /tmp/mara72-target/debug/mara with debug information and incremental builds disabled; temporary storage was /var/tmp/mara-upgrade-tmp.
:::
