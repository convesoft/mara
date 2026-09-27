# Trace matrices and request-local checks

Matrices explain selected current-state obligations without changing project
policy or canonical source. Native evaluation follows [validation](validation.mara.md).

:::mara requirement REQ-TRACE-MATRIX
:mid: 01M2FX4BTAHMMPEEPX6JK418ZS
:title: Generate matrices that explain selected trace coverage
:status: draft
:kind: functional
:derives_from: SCN-READ-TRACE-VIEW

Generate a read-only matrix from selected canonical items and declared obligations. Preserve selection, evaluation identity, relationship meaning and source navigation. Show covered, failed, not-applicable and unavailable states, including non-qualifying targets and second-hop gaps, using the same native semantics as validation.

Allow reusable request-local checks to bind an exact caller-supplied revision to evidence criteria without modifying the YAML or claiming evidence authenticity. Distinguish structural coverage, verification definitions and recorded execution. Report per-evaluation counts and explicit continuation/incompleteness; do not invent a global coverage percentage.
:::

:::mara requirement REQ-BOUNDED-TRACE-CHAINS
:mid: 01M2FX4BSP29ZW778MW2EYV4XT
:title: Evaluate explicit relationship chains with bounded explanations
:status: draft
:kind: functional

Rules and matrices follow finite explicit relationship steps, directions and target conditions. Preserve semantic relation identity and source provenance; unrelated paths cannot supply coverage. Distinguish immediate linkage from fulfillment of downstream obligations.

Reject recursive shape references and unbounded paths under the configured definition limits. Distinguish incomplete evaluation from complete failure and output continuation. No logical work counter, exhaustive simple-path enumeration or deepest-leaf explanation is required. Results and continuation are deterministic for unchanged inputs.
:::

:::mara requirement REQ-TRACE-COVERAGE
:mid: 01M2FX4BRV92A6G1GXX5DDAZ5P
:title: Count only relationships that satisfy the declared obligation
:status: draft

Pending capability review authoring.
:::

:::mara design DES-TRACE-VIEW-INTERFACES
:mid: 01M2JP3PJV8WZKNR1WWF0GJMS4
:title: Generate bounded traceability matrices from explicit selections
:status: draft

Pending capability review authoring.
:::

:::mara design DES-TRACE-CHECK-BINDING
:mid: 01M3H7KNT9NHTH8QMHTRKDF4SG
:title: Bind request-local check literals without changing project policy
:status: draft
:kind: interface
:satisfies: REQ-TRACE-MATRIX

A matrix check supplies nonempty project-relative YAML files and one expanded IRI naming a targetless node shape. Reject path/property roots, targetClass, whenShape and paths on the designated shape. Validate compatibility with selected flavours and evaluate only that root and its referenced shapes. Other targeted shapes in the files do not become enabled project policy.

Within hasValue or an in-list entry, exactly {parameter: NAME} binds caller text. Names match [A-Za-z_][A-Za-z0-9_]* and are case-sensitive. CLI --param NAME=VALUE and MCP check.parameters supply exact text, including empty values; never parse a value as YAML, IRI, datatype or Git ref. Repeated occurrences share one binding. Preserve authored YAML and ordinary literals.

Reject invalid names/placeholders, missing/non-text/unused bindings, duplicate CLI names and parameters without a request check as invalid_argument. Every placeholder in the supplied files needs a value; each supplied binding must be used by the selected root or a referenced shape. Persisted enabled rules still reject placeholders. Include resolved literals in explanations and parameter values in cursor identity. Revision evidence checks select recorded evidence for the supplied text; they neither execute tests nor authenticate evidence.
:::
