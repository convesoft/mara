# Corpus conformance and validation

Source-level checks are a shared prerequisite for validation and authoring.
Project/item validation combines independently recoverable source diagnostics
with configured current-state rules and structural relation policies.

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

Source conformance checks identities, valid flavour/field declarations, required bodies, metadata, typed relations and discovery references. Numeric field values must be finite. Check every valid authored MID, including repeated entries, so secondary duplicate identities remain visible. Use schema recovery flags to suppress only checks dependent on invalid declarations, and item recovery flags to avoid inventing missing fields or bodies. Proven ambiguity remains an error; report a missing item target only when discovery is complete, including targets authored in code comments.

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

This method checks library source conformance. [[VER-PROJECT-VALIDATION]] checks validation transports, [[VER-POLICY-VALIDATION]] checks rules and graph policies, and [[VER-MID-AND-RECOVERY]] checks identity backfill and recovery writes.
:::

:::mara evidence EVD-CORPUS-CONFORMANCE
:mid: 01M3H30DMNEMAPG8QKBHHF27AQ
:title: Corpus conformance and prior capability regressions pass
:status: retired
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

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity with zero diagnostics. Installed-tool intent passed two selected roots; realization and verification each passed three selected roots, consuming all pages. These authoring/trace checks do not establish candidate project validation. This execution checked library conformance, not project/item validation transports, rule/graph evaluation, MID backfill or recovery writes.
:::

:::mara requirement REQ-RELATION-CARDINALITY
:mid: 01M2FX4BS193R12Z27E3M7CQZQ
:title: Check declared relationship cardinality
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Constrain minimum/maximum semantic edges of a declared relation at eligible outgoing, incoming or symmetric endpoints. Repeated metadata/inline occurrences, ID/MID targets and inverse spellings count once; distinct relation kinds remain distinct. Preserve declared warning/error severity and report actual counts and bounds. Conditional qualifying-target counts belong to current-state rules.
:::

:::mara requirement REQ-RELATION-CYCLE-POLICY
:mid: 01M2FX4BS8BH26TSW1XWNWAWE4
:title: Check cycles only where the schema prohibits them
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Prohibit cycles only for directed relations whose schema declares acyclicity. Resolve inverse aliases first; a self-loop is a cycle. Report involved items and a source-linked witness. Unconstrained relations, mentions, containment and symmetric associations do not acquire an acyclicity policy. External endpoints are terminal and cannot close an item cycle.
:::

:::mara requirement REQ-CURRENT-STATE-RULES
:mid: 01M2FX4BSGPX5K6KBMK9RQ1J1Y
:title: Validate project-defined item and lifecycle obligations
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Evaluate project-declared named rules over current item fields and semantic relationships after direct edits or structured authoring. Project-defined status fields have no built-in lifecycle. Class/path selection and applicability conditions restrict obligations; non-applicable items do not fail. Support conditional fields and qualified relationship coverage through finite explicit shape chains.

Invalid definitions or corpus source/identity/field/reference prerequisites must not become successful checks. Skip policy evaluation over an invalid or partial corpus, including item-targeted requests, retain original diagnostics and report unavailability. An encountered native evaluation error is unavailable rather than a failed or passed obligation. Mutation uses source conformance independently of these policies; validation is read-only.
:::

:::mara requirement REQ-TRACE-DIAGNOSTICS
:mid: 01M2FX4BSX7T5C5GHF628WAHD1
:title: Expose stable diagnostics and project-defined rule severity
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

CLI and MCP expose stable diagnostic codes, severity, affected item/source or configuration location, actionable messages and full-target validity. Rule failures name the rule and reported obligation with authored source and available relation/count details; consumers classify by code/severity, not prose.

Warnings remain visible without invalidating a complete result. Source/configuration errors and unavailable evaluation cannot be downgraded to warnings. Reporting path filters and pagination never hide failures from the full summary or CLI exit status. Distinguish operation errors from completed validation with failures; preserve deterministic continuation for unchanged inputs.
:::

:::mara design DES-TRACE-GRAPH-CONSTRAINTS
:mid: 01M2JP04Z4R4RJP4MWYNN442Z4
:title: Compose structural graph policies with conditional rules
:status: accepted
:kind: behavior
:satisfies: REQ-RELATION-CARDINALITY
:satisfies: REQ-RELATION-CYCLE-POLICY

Evaluate declared cardinality and acyclicity after complete source conformance, using the canonical semantic relation graph. When any prerequisite is invalid or discovery is incomplete, emit project-scoped evaluation_unavailable and retain blockers, including on item requests.

Count each distinct incident semantic edge once in its declared direction for eligible flavours; a symmetric self-edge counts once. Respect same-flavour eligibility. External targets contribute outgoing counts; code sources contribute eligible incoming counts. Item validation selects the requested endpoint while using the complete graph.

For acyclicity, compute strongly connected components of each constrained directed item graph after alias normalization. Report each cyclic component once in stable MID order, including self-loops, with member IDs/MIDs and a deterministic cycle of authored occurrence references. An item request reports its component using that item as focus. Apply the declaration's severity; never traverse external endpoints as items.
:::

:::mara design DES-TRACE-DIAGNOSTIC-INTERFACE
:mid: 01M2JP1YNBM7T7G9E67CV0RP6V
:title: Bound evaluation and expose stable validation diagnostics
:status: accepted
:kind: interface
:satisfies: REQ-PROJECT-VALIDATION
:satisfies: REQ-TRACE-DIAGNOSTICS

CLI project validate accepts repeatable --path reporting selectors; item validate takes an exact human ID or canonical MID. MCP project_validate and item_validate share format-1 results with schema validation: project, target, valid, evaluation_complete, diagnostics, summary, selection, has_more and next_cursor. Summaries cover the whole target before filtering/pagination; valid requires complete evaluation and zero errors. Incomplete counts are lower bounds (counts_exact:false). Schema-only validation loads rule definitions without item evaluation.

Recover independently usable configuration, schema and source. Associate diagnostics with unambiguous item identities, retain unavailable coordinates as absent and preserve path/line aliases. Item checks retain selected source problems; when whole-corpus policy prerequisites fail, include their actual blockers. Prove absence only with complete discovery. Source errors retain their typed classification; emit one stable violation per failed item/rule using authored location, shape, component, focus and value ordering. Unavailable native counts remain null; native error details are sanitized.

Report diagnostic pages with limit 1–100 (default 20), at most 65,536 serialized bytes. Repeat unchanged inputs until has_more:false. Bind cursors to project/schema, accepted rule sources, discovered document/code bytes, adapter assets, file-only code and options; changes invalidate them. A single oversized diagnostic/envelope returns output_limit without silently dropping data. Invalid arguments, stale cursors and I/O preventing results use format-1 operation errors; CLI fails on invalid results/errors, MCP invalid validation is successful tool data.

Evaluation has no logical work counter or timeout guarantee. Retain finite rule-definition limits from [[DES-SCHEMA-RULE-DEFINITIONS]]. Validation never writes source, fetches external addresses or performs Git operations.
:::

:::mara decision ADR-NATIVE-SHACL-ADAPTER
:mid: 01M37AKP5PVWSECJR2T28VFFRS
:title: Use unmodified native SHACL through a public adapter
:status: accepted
:justifies: REQ-CURRENT-STATE-RULES

Use the pinned unmodified native SHACL engine through its public adapter API rather than patching vendor code or reimplementing constraint semantics. Retain Mara's authored vocabulary/type checks and deterministic diagnostic selection at the boundary. Keep a nested error ledger because native logical/qualification validators can swallow an error into nonconformance; any encountered error must remain unavailable. Expose counts only from completed native outcomes. Constraint-semantic changes require a separate compatibility decision; matrix presentation remains a separate projection.
:::

:::mara design DES-CURRENT-STATE-EVALUATION
:mid: 01M3H6VK886BQ3YVVJDJA2GAXC
:title: Project typed current state into native rule evaluation
:status: accepted
:kind: behavior
:satisfies: REQ-CURRENT-STATE-RULES
:satisfies: REQ-TRACE-DIAGNOSTICS

After complete source conformance and valid definitions, project each item as a MID-keyed node with its flavour and typed custom fields. Deduplicate RDF field values and semantic edges; symmetric relations project both directions. Code and external endpoints use opaque local identities with no item fields or invented remote state. No network reads occur.

For each selected item and enabled root, apply targetClass and normalized document paths, then whenShape. A nonconforming condition skips the obligation; an engine error makes it unavailable. Evaluate applicable obligations with the pinned native SHACL adapter and nested-error ledger from [[ADR-NATIVE-SHACL-ADAPTER]]. A finite failed obligation is complete; an encountered engine error is not. Native evaluation owns literal, logical, qualification and path semantics; definition validation bounds shape/relationship depth.

Select one reported violation deterministically by authored source location, shape, component, focus and value. Preserve root severity, root identity, reported obligation location/message and available native counts; do not infer missing cached results or parse native prose. Whole-corpus prerequisites apply even when one item is selected. Matrix observation and presentation are separate capabilities.
:::

:::mara verification VER-PROJECT-VALIDATION
:mid: 01M3H72PE1G21HP7Z3PQXDQ012
:title: Check full-context validation and bounded diagnostic transports
:status: accepted
:method: test
:level: system
:verifies: REQ-PROJECT-VALIDATION
:verifies: REQ-TRACE-DIAGNOSTICS
:verifies: DES-TRACE-DIAGNOSTIC-INTERFACE

Run cargo test --locked --test project_validation in isolated temporary Git projects. Preserve distinct recovery cases for malformed configuration/schema/items, title/metadata errors, identity ambiguity, missing-item proof, unreadable source and directory walks. Verify real CLI/MCP envelopes, human output, exit status, item context, stable codes/locations, full-target hidden failures, bounded pages, snapshot/option invalidation, operation errors and oversized records. Source must remain unchanged. Run the full regression suite as regressions; complete formatting, Clippy and canonical validation.
:::

:::mara verification VER-POLICY-VALIDATION
:mid: 01M3H72VBM2NA5FPWVHYAPSZXG
:title: Check native current-state rules and semantic graph policies
:status: accepted
:method: test
:level: system
:verifies: REQ-CURRENT-STATE-RULES
:verifies: REQ-RELATION-CARDINALITY
:verifies: REQ-RELATION-CYCLE-POLICY
:verifies: DES-CURRENT-STATE-EVALUATION
:verifies: DES-TRACE-GRAPH-CONSTRAINTS

Run cargo test --locked --test policy_validation and the native engine nested-error regression. Exercise real CLI/MCP lifecycle and qualified relationship failures followed by repair; typed literals and nested class/path selection; messages, warning/error severity, invalid prerequisites and continuation; bounded multi-hop chains and allowed cycles; distinct semantic cardinality, aliases, symmetric/self/external edges and constrained cycle witnesses. Reuse standard fixture helpers with fixture-owned vocabulary/rules and isolated Git/configuration. Rule evaluation never dereferences external URLs. Corpus loading invokes trusted configured indexers under [[DES-CODE-TRACEABILITY]]; Mara does not write source. Keep definition-only regressions in schema_validation; matrix output remains separate. Run the full regression suite and selected traceability at the actual implementation revision.
:::

:::mara evidence EVD-PROJECT-POLICY-VALIDATION
:mid: 01M3H7A7HMV3ZA1VQRS321M020
:title: Full-context validation and native policy evaluation pass
:status: retired
:result: passed
:captured_at: 2026-09-27T10:41:55Z
:subject_revision: 7387584738ec86d2f810e73c2d0041317cb2fdc6
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

At the subject revision, cargo test --locked --all-targets passed 332 tests across 24 suites with zero failures. Two ignored process helpers were each exercised at three interruption boundaries by passing parent tests. The new suites passed 52 project/item/source diagnostic groups and 17 policy groups; the native nested-error ledger regression also passed. Tests use disposable Git projects, fixture-owned rule/schema/configuration inputs and local adapters. The complete suite, including the read-only self-hosting check, ran without source/documentation changes.

cargo fmt --all --check and cargo clippy --locked --all-targets -- -D warnings passed. The candidate CLI validated this complete repository and REQ-CURRENT-STATE-RULES with valid:true, evaluation_complete:true, zero errors/warnings and no remaining page. Real CLI/MCP fixture parity covers failures, repairs, item scope, source recovery, warning severity, native rule/graph outcomes, diagnostic filtering, stale cursors and output limits.

Installed authoring-tool schema/project validation passed without diagnostics. Selected intent passed five roots across two pages; realization and verification each passed eight roots across three pages, consuming all continuation. This execution covers validation at the subject revision. It does not establish matrix operations, request-local bindings or packaging.
:::
