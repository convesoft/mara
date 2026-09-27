# Trace matrices and request-local checks

Matrices explain selected current-state obligations without changing project
policy or canonical source. Native evaluation follows [validation](validation.mara.md).

:::mara requirement REQ-TRACE-MATRIX
:mid: 01M2FX4BTAHMMPEEPX6JK418ZS
:title: Generate matrices that explain selected trace coverage
:status: accepted
:kind: functional
:derives_from: SCN-READ-TRACE-VIEW

Generate a read-only matrix from selected canonical items and declared obligations. Preserve selection, evaluation identity, relationship meaning and source navigation. Show covered, failed, not-applicable and unavailable states, including non-qualifying targets and second-hop gaps, using the same native semantics as validation.

Allow reusable request-local checks to bind an exact caller-supplied revision to evidence criteria without modifying the YAML or claiming evidence authenticity. Distinguish structural coverage, verification definitions and recorded execution. Report per-evaluation counts and explicit continuation/incompleteness; do not invent a global coverage percentage.
:::

:::mara requirement REQ-BOUNDED-TRACE-CHAINS
:mid: 01M2FX4BSP29ZW778MW2EYV4XT
:title: Evaluate explicit relationship chains with bounded explanations
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Rules and matrices follow finite explicit relationship steps, directions and target conditions. Preserve semantic relation identity and source provenance; unrelated paths cannot supply coverage. Distinguish immediate linkage from fulfillment of downstream obligations.

Reject recursive shape references and unbounded paths under the configured definition limits. Distinguish incomplete evaluation from complete failure and output continuation. No logical work counter, exhaustive simple-path enumeration or deepest-leaf explanation is required. Results and continuation are deterministic for unchanged inputs.
:::

:::mara requirement REQ-TRACE-COVERAGE
:mid: 01M2FX4BRV92A6G1GXX5DDAZ5P
:title: Count only relationships that satisfy the declared obligation
:status: accepted
:kind: functional
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

A coverage rule specifies relation kind, orientation and qualifying target conditions. Support minimum/maximum qualifying counts and obligations applying to every selected related endpoint. Distinguish at least one qualifying target from all targets qualifying.

For an accepted requirement needing one accepted verification, a draft verification does not count; adding an accepted verification can satisfy the rule while the draft remains, unless a separate all-targets obligation forbids it. Count each semantic relationship once regardless of authored occurrences. Untyped mentions and unrelated relations cannot satisfy the obligation.

Distinguish structural coverage, verification definitions and recorded execution evidence. A code, test or ticket link alone never establishes implementation correctness or successful execution.
:::

:::mara design DES-TRACE-VIEW-INTERFACES
:mid: 01M2JP3PJV8WZKNR1WWF0GJMS4
:title: Generate bounded traceability matrices from explicit selections
:status: accepted
:kind: interface
:satisfies: REQ-TRACE-MATRIX
:satisfies: REQ-BOUNDED-TRACE-CHAINS
:satisfies: REQ-TRACE-COVERAGE
:satisfies: REQ-SURFACE-PARITY

Matrices are disposable read-only projections of one loaded project. Reuse [[DES-CURRENT-STATE-EVALUATION]], [[DES-CANONICAL-TRACE-RELATIONS]] and [[DES-TRACE-DIAGNOSTIC-INTERFACE]]. Selection belongs to the request; no saved view or source mutation is introduced.

## Selection and operation

CLI `trace matrix` and MCP `trace_matrix` accept exact ids, flavours, custom-field text equality and document/subtree paths under the item-filter contract. OR values within a filter and intersect filter categories. Require at least one filter or explicit all:true alone. Normalize and deduplicate selection; reject unknown vocabulary/IDs and invalid paths. A valid empty match returns an empty view. Selection chooses roots; related endpoints outside it remain available for evaluation.

Supply either nonempty expanded IRIs naming enabled root rules or one request-local check, never both. Named-rule class/path/applicability conditions intersect the selected roots. Request checks and literal parameters follow [[DES-TRACE-CHECK-BINDING]]; they do not enable project policy. CLI flags are --all, repeatable --id/--flavour/--field/--path/--rule, --check-file, --shape, --param, --limit and --cursor. MCP accepts those selections, rules or check, limit, cursor and optional render:"markdown".

CLI JSON and MCP share trace format_version:1, kind:matrix, normalized selection, evaluation_complete, summaries, records, has_more and next_cursor. CLI text uses the same Markdown page as MCP render:"markdown". Complete policy failures are data: CLI exits zero and MCP isError is false. Unavailable evaluation returns an incomplete domain result and CLI exit one; operation errors use the validation error family. A matrix does not claim whole-project validity.

## Records and explanation

Emit one result record per selected root/evaluation pair, including not_applicable. Evaluation identifies kind rule or check and expanded shape IRI. Applicable outcomes expose source-linked check records: opaque snapshot-bound reference, root, evaluation, obligation shape/component/source, ordered context, parent reference, state, condition, counts, every, inspection and reported obligation. Component is the sorted array of authored SHACL constraint-component IRIs; state describes the whole shape. Condition.components retains authored parameters and resolved literals. Unknown native outcomes remain absent or null.

An edge record identifies its check, canonical semantic edge, endpoint-facing label, direction, endpoint, outside_selection, qualification/every outcomes, occurrence_count and inspection. Deduplicate multiple assertions; inspect their locations through relation get. Item endpoints have discovery descriptors; code and external endpoints retain their exact relationship identities without invented item fields. Code/external endpoints are outside root selection. Preserve native qualifier/every outcomes when evaluated.

Counts describe the immediate obligation: selected, qualifying, minimum and maximum, with unknown totals and omitted bounds null. Context contains ordered canonical edges, at most eight hops. Local-field inspection identifies focus and field, total authored value count, first value limited to 256 characters, truncation and source location; absent fields have null value/source. Nested details come from explicitly evaluated or public native outcomes; exhaustive traces and deepest failing leaves are not required.

Order roots by path/start byte, evaluations by expanded IRI, checks by authored source/shape/component and edges by canonical endpoint. Invalid prerequisites produce issue records with validation diagnostics and skipped evaluation. Summaries repeat whole-view selected, not_applicable, passed, failed, unavailable and counts_exact for each evaluation; incomplete counts are lower bounds. No global percentage is computed.

## Rendering and continuation

Markdown presents source-linked roots, obligations and endpoint-facing relations. Display a code endpoint's exact reference, preserving descriptor punctuation and literal backticks, linked to its project-relative file. External endpoints display their address and terminal kind. Neither receives an invented item ID, status or item source location.

Return consecutive record pages with default limit 20 and maximum 100, within 65,536 serialized bytes including requested Markdown. An indivisible record or envelope that cannot fit fails explicitly without skipping it. Follow next_cursor until has_more:false. Bind continuation to project/schema, source and code input snapshots, accepted rule/check sources, excluded invalid sources, file-only code bytes, normalized selection, bindings, limit and render mode. Changed inputs or malformed/out-of-range cursors require restart. Check references can connect records across pages but are not durable identities.
:::

:::mara design DES-TRACE-CHECK-BINDING
:mid: 01M3H7KNT9NHTH8QMHTRKDF4SG
:title: Bind request-local check literals without changing project policy
:status: accepted
:kind: interface
:satisfies: REQ-TRACE-MATRIX

A matrix check supplies nonempty project-relative YAML files and one expanded IRI naming a targetless node shape. Reject path/property roots, targetClass, whenShape and paths on the designated shape. Validate compatibility with selected flavours and evaluate only that root and its referenced shapes. Other targeted shapes in the files do not become enabled project policy.

Within hasValue or an in-list entry, exactly {parameter: NAME} binds caller text. Names match [A-Za-z_][A-Za-z0-9_]* and are case-sensitive. CLI --param NAME=VALUE and MCP check.parameters supply exact text, including empty values; never parse a value as YAML, IRI, datatype or Git ref. Repeated occurrences share one binding. Preserve authored YAML and ordinary literals.

Reject invalid names/placeholders, missing/non-text/unused bindings, duplicate CLI names and parameters without a request check as invalid_argument. Every placeholder in the supplied files needs a value; each supplied binding must be used by the selected root or a referenced shape. Persisted enabled rules still reject placeholders. Include resolved literals in explanations and parameter values in cursor identity. Revision evidence checks select recorded evidence for the supplied text; they neither execute tests nor authenticate evidence.
:::

:::mara verification VER-TRACE-MATRIX
:mid: 01M3HBVG1NRC6NK3MV3376CER8
:title: Check matrix explanations, continuation and revision evidence
:status: accepted
:method: test
:level: integration
:verifies: REQ-TRACE-MATRIX
:verifies: REQ-BOUNDED-TRACE-CHAINS
:verifies: REQ-TRACE-COVERAGE
:verifies: REQ-SURFACE-PARITY
:verifies: DES-TRACE-VIEW-INTERFACES
:verifies: DES-TRACE-CHECK-BINDING

Run cargo test --locked --test trace_matrix against disposable projects with fixture-owned vocabulary, rules, sources and Git/configuration state. Exercise the real CLI and stdio MCP for normalized selections, valid empty matches, rule/check exclusivity, applicability, passed/failed/unavailable states, source-linked checks/edges, local field inspection, native literal counts, every/qualification behavior and code/external terminals.

Verify second-hop gaps, bounded context, complete record continuation with unchanged summaries, count and byte budgets, oversized records, stale cursors after source/schema/code/asset/excluded-source/check/binding/render changes, and sanitized operation errors. Check exact literal parameter binding, empty text, in-list reuse, invalid/missing/unused/non-text/duplicate parameters and persisted-policy placeholder rejection without source writes.

Require real CLI/MCP Markdown to preserve the exact SCIP reference and link to its file, including literal backticks. Preserve item/external rendering. Run the engineering profile workflow from draft knowledge through accepted coverage and exact-revision evidence, distinguishing missing or historical evidence from the selected result.

Run formatting, Clippy, the full relevant suite, complete candidate project validation and selected candidate intent/realization/verification/execution matrices with every continuation page consumed. Record actual tested-revision evidence; traces neither execute tests nor authenticate evidence.
:::
