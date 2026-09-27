# Corpus conformance and validation

Source-level checks are a shared prerequisite for validation and authoring.
Project/item validation transports, rules and graph policies remain separate
rebuild increments.

:::mara requirement REQ-PROJECT-VALIDATION
:mid: 01M1PXP2KGWKQRXBB29DX5D7G1
:title: Validate a selected item or the full project
:status: accepted
:kind: functional
:derives_from: SCN-START-STRUCTURED-PROJECT

`item validate <id>` checks one item in project context; `project validate` checks the complete corpus. Validation covers project/schema configuration, syntax, known flavours, ID prefixes and uniqueness, MID presence/format/placement/uniqueness, required fields and bodies, metadata, relation declarations and targets, and supported wiki mentions by human ID or MID. Broken relations and mentions are errors. Report every independently discoverable diagnostic with an actionable source location; skip only checks whose prerequisites are invalid.

Optional project-relative reporting paths select document diagnostics without narrowing the configured context. Whole-project validity and CLI exit status include omitted diagnostics; project/schema diagnostics remain visible.
:::

:::mara design DES-CORPUS-CONFORMANCE
:mid: 01M3H2QK7C533M2FRS06JB3YPT
:title: Recover source and check independently available corpus facts
:status: accepted
:kind: behavior
:satisfies: REQ-PROJECT-VALIDATION
:satisfies: REQ-DURABLE-ITEM-IDENTITY

The recovering corpus loader composes document recovery with configured local code discovery. Preserve readable source and item context after independent errors; missing/unreadable source makes discovery incomplete. Without a usable schema, syntax-only recovery does not load code adapters.

Source conformance checks identities, valid flavour/field declarations, required bodies, metadata, typed relations and discovery references. Check every valid authored MID, including repeated entries, so secondary duplicate identities remain visible. Use schema recovery flags to suppress only checks dependent on invalid declarations, and item recovery flags to avoid inventing missing fields or bodies. Proven ambiguity remains an error; report a missing item target only when discovery is complete, including targets authored in code comments.

Retain stable diagnostic codes and original source spans; order by path, line and message. Associate code problems with their target's exact human ID and MID when resolvable. Reads never repair source. These checks do not execute conditional rules or graph policies and cannot alone establish project validity.
:::

:::mara verification VER-CORPUS-CONFORMANCE
:mid: 01M3H2QPZVCNH48VPK7X66W3JZ
:title: Check source conformance and recovery independently of transports
:status: accepted
:method: test
:level: integration
:verifies: REQ-PROJECT-VALIDATION
:verifies: REQ-DURABLE-ITEM-IDENTITY
:verifies: DES-CORPUS-CONFORMANCE

Run `cargo test --locked --test corpus_validation` in isolated temporary projects with fixture-owned documents, Git state and adapters. Check identity format/placement/bijection and secondary MID diagnostics; field types/required/repeated values; schema and item recovery; item, inverse, external and code endpoints; complete versus incomplete missing-target behavior; and unchanged source bytes. Run existing CLI/MCP suites, formatting and Clippy as regressions.

This method verifies the shared library prerequisite. It does not execute candidate project/item validation transports, conditional rules, graph policies, MID backfill or recovery writes. Preserve their baseline tests for later checkpoints.
:::

:::mara evidence EVD-CORPUS-CONFORMANCE
:mid: 01M3H30DMNEMAPG8QKBHHF27AQ
:title: Corpus conformance and prior capability regressions pass
:status: accepted
:result: passed
:captured_at: 2026-09-27T09:26:25Z
:subject_revision: dc951880d771e9f3c679d17907142832b3bbc102
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

At `dc951880d771e9f3c679d17907142832b3bbc102`, `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets` passed: 174 tests, none failed or ignored. Fourteen corpus-conformance groups cover identity diagnostics, independent schema/source recovery, typed fields, relation endpoints, source preservation and code-problem association. Existing real CLI/MCP tests remain passing.

The targeted unreadable-document regression failed before the correction: a code marker reported its target missing while discovery was incomplete. It passes with complete-missing and resolved-target counterparts after applying the same completeness guard used by item relations.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity with zero diagnostics. Installed-tool intent passed two selected roots; realization and verification each passed three selected roots, consuming all pages. These authoring/trace checks do not establish candidate project validation. The new implementation is a library prerequisite; project/item validation transports, rule/graph evaluation, MID backfill and write recovery remain pending.
:::
