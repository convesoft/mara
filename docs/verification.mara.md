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

:::mara evidence EVD-CANONICAL-CORPUS
:mid: 01M3HG6MM70E44P5NXQV9GHF8T
:title: Current canonical corpus passes documentation inspection
:status: accepted
:result: passed
:captured_at: 2026-09-27T13:17:07Z
:subject_revision: b18a94116ad67c9fd54d46479335e5b5fdfe29df
:evidences: VER-CANONICAL-DOCUMENTATION

At subject revision `b18a94116ad67c9fd54d46479335e5b5fdfe29df`, manual review covered canonical product documents and standalone skill guidance for understandable current behavior, canonical ownership of facts, accurate examples and meaningful references. Supported schema, identity, code-indexing and recovery behavior is stated directly. Every integration-test suite named by the verification definitions exists in the checkout.

Mara inspection consumed all 118 current items across 2 pages and the complete documentation verification method. A source comparison preserved every surviving product identity, metadata field and relationship. No retired or superseded evidence records, evidence archives, tombstones or references to removed records remain in the corpus. The retained matrix execution evidence keeps its original capture date, subject revision, results and verified scope; this documentation inspection does not relabel that application test run.

The candidate returned complete, valid schema and project results with zero diagnostics across 1 schema page and 1 project page. Selected intent passed `REQ-CANONICAL-SOURCE` and `REQ-MID-BACKFILL` across 1 page. Verification coverage passed `REQ-CANONICAL-SOURCE`, `REQ-MID-BACKFILL`, `DES-FLAVOUR-AUTHORING-GUIDANCE`, `DES-SCHEMA-VALIDATION` and `DES-CODE-TRACEABILITY` across 2 pages with exact counts and no failed or unavailable result. Every continuation page was consumed. The retained matrix evidence also passed its selected execution check at subject revision 1e57232f464a838bf34839840b2662d9ed7524e0 across one complete page. Manual prose review separately assessed clarity and duplication.

`git diff --check` and the documentation-only scope check passed. No application tests were rerun for this documentation revision. Environment: Linux x86_64, Rust/rust-analyzer 1.97.1, candidate `/tmp/mara72-target/debug/mara`; CARGO_TARGET_DIR=/tmp/mara72-target, CARGO_PROFILE_DEV_DEBUG=0, CARGO_PROFILE_TEST_DEBUG=0, CARGO_INCREMENTAL=0. This inspection does not establish packaged distribution or release publication.
:::
