# Validation

Validation is Mara's primary bootstrap value. One command turns source files
into a checked semantic project and reports actionable problems without hiding
later issues behind the first failure.

## Validation commands and pipeline

:::req m_01KY7Y9R6PG0TY1XWHQG7ZNSH5
:id: REQ-CHECK-PIPELINE
:title: mara check shall validate the complete project pipeline
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-CHECK-WORKTREE

`mara check` shall discover the project, load and validate configuration and
schema, discover content, parse complete documents and items, validate
identities and fields, resolve references and canonical relations, evaluate all
configured graph rules, and report one project summary.
:::

:::req m_01KY7Y9R6QR2S81W4TPT4N05E8
:id: REQ-SCHEMA-CHECK-COMMAND
:title: Mara shall validate a schema independently of content
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-VALIDATE-PROJECT

`mara schema check` shall load project configuration and validate the referenced
schema and its meta-model without requiring content files to parse successfully.
:::

:::req m_01KY7Y9R6RX2E5SF2G6XD77KKZ
:id: REQ-VALIDATION-PHASES
:title: Mara shall preserve errors from independent validation phases
:status: approved
:level: software
:kind: quality
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

A failure shall prevent only dependent work. Mara shall continue evaluating
independent files and checks whenever results remain unambiguous, while clearly
marking checks skipped because a prerequisite model could not be constructed.
:::

## Structured diagnostics

:::req m_01KY7Y9R6SEJXXA8DHJ96FZCRT
:id: REQ-DIAGNOSTIC-MODEL
:title: Mara shall represent every issue as structured diagnostic data
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: GOAL-AGENT-READY

A diagnostic shall contain a stable code, severity, message, primary source
span, optional related spans, MID and display ID when known, field or relation
context when applicable, and machine-readable details specific to the code.
:::

:::req m_01KY7Y9R6T2BAC5KX6BCJD4JKT
:id: REQ-DIAGNOSTIC-COLLECTION
:title: Mara shall collect all independently discoverable diagnostics
:status: approved
:level: system
:kind: usability
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall not stop at the first validation error. It shall aggregate every
issue that can be established safely from the available project model and print
the total counts by severity.
:::

:::req m_01KY7Y9R6VVX7NH25ACQW6F7FD
:id: REQ-DIAGNOSTIC-ORDER
:title: Mara shall order diagnostics deterministically
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-AUDITABLE

Diagnostics shall be sorted by normalized project-relative path, source
position, severity, code, and stable detail keys so repeated checks of unchanged
input produce the same order regardless of filesystem enumeration.
:::

:::req m_01KY7Y9R6WYKT2NRCE6HSZNRQM
:id: REQ-DIAGNOSTIC-OUTPUT
:title: Mara shall provide human and stable JSON diagnostic output
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-VALIDATE-PROJECT
:derives_from: GOAL-AGENT-READY

Validation commands shall support `--format human|json`. Human output shall be
concise and source-oriented; JSON field names, diagnostic codes, and severity
values shall be treated as versioned compatibility surfaces.
:::

:::req m_01KY7Y9R6XVMG0DZBC8BYNMNQ7
:id: REQ-VALIDATION-EXIT-CODES
:title: Mara shall distinguish validation and operational failures
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: SCN-CHECK-WORKTREE

Commands shall return exit code zero on success, a dedicated nonzero code when
the project was processed but contains failing diagnostics, and a different
nonzero code when operational or configuration failure prevented the requested
operation. Configured warning escalation shall affect the validation result.
:::

## Structural and semantic checks

:::req m_01KY7Y9R6YCTVMQQWWMVZ1EBAR
:id: REQ-VALIDATE-ITEMS
:title: Mara shall validate item structure and field values
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall validate flavour existence, MID syntax and uniqueness, display-ID
requiredness, pattern and uniqueness, title and body requiredness, known
metadata keys, field requiredness, scalar conversion, enum membership, string
patterns, and repetition constraints.
:::

:::req m_01KY7Y9R6ZCF0RNK98NBAGGTE1
:id: REQ-VALIDATE-RELATIONS
:title: Mara shall validate reference and relation integrity
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall validate reference resolution, relation names and inverse names,
source and target kinds, external schemes, self-reference, cardinality, same-
flavour requirements, and duplicate occurrence policy before exposing a valid
canonical graph.
:::

:::req m_01KY7Y9R70SHZHBANNGYKGZ6B4
:id: REQ-VALIDATE-RULES
:title: Mara shall evaluate only schema-configured traceability rules
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC

After structural resolution, Mara shall evaluate the project's declarative
conditional field, relation, cardinality, and lifecycle traceability rules with
their configured severities. The engine shall add no implicit methodology rule.
:::

:::req m_01KY7Y9R71V4BHDGJ26VFJPKZS
:id: REQ-VALIDATE-CYCLES
:title: Mara shall detect cycles only for relations declared acyclic
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-TRACEABILITY

For each relation marked acyclic, Mara shall report a diagnostic containing a
concrete cycle path. Cycles in relations without that constraint shall remain
valid and shall not be inferred as failures.
:::

:::req m_01KY7Y9R72Q1WHTMXCDJBVYP25
:id: REQ-VALIDATE-ORPHANS
:title: Mara shall provide schema-configured orphan detection
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-TRACEABILITY

The orphan rule shall let a schema select flavours, qualifying fields or
statuses, relation kinds that count as connectivity, and issue severity. Mara
shall not universally classify every relation-free item as an orphan.
:::

:::req m_01KY7Y9R73JEZE6Z6C295NBX4J
:id: REQ-VALIDATION-COMPATIBILITY
:title: Mara shall version diagnostic and validation contracts
:status: approved
:level: system
:kind: constraint
:priority: should
:derives_from: GOAL-AUDITABLE

Pre-1.0 releases may introduce breaking diagnostic or persisted-format changes
only with an explicit format or command-interface version change and release
notes. Mara shall fail on unsupported persisted versions instead of silently
migrating or guessing.
:::

## Design and risk

:::design m_01KY7YA2CJKTWWKAYD9WHTY9GG
:id: DES-VALIDATION-PIPELINE
:title: Dependency-aware validation pipeline
:status: accepted
:kind: component
:satisfies: REQ-CHECK-PIPELINE
:satisfies: REQ-SCHEMA-CHECK-COMMAND
:satisfies: REQ-VALIDATION-PHASES
:satisfies: REQ-DIAGNOSTIC-MODEL
:satisfies: REQ-DIAGNOSTIC-COLLECTION
:satisfies: REQ-DIAGNOSTIC-ORDER
:satisfies: REQ-DIAGNOSTIC-OUTPUT
:satisfies: REQ-VALIDATION-EXIT-CODES
:satisfies: REQ-VALIDATE-ITEMS
:satisfies: REQ-VALIDATE-RELATIONS
:satisfies: REQ-VALIDATE-RULES
:satisfies: REQ-VALIDATE-CYCLES
:satisfies: REQ-VALIDATE-ORPHANS
:satisfies: REQ-VALIDATION-COMPATIBILITY

Validation stages shall exchange typed partial results and structured
diagnostics. A stage declares its prerequisites, allowing unrelated documents
and checks to continue while dependent graph rules are skipped when the global
model is invalid.
:::

:::decision m_01KY7YA2CKFNW44M0GKP3E4WZ0
:id: ADR-0006
:title: Treat diagnostics as an API
:status: accepted
:kind: architecture
:justifies: DES-VALIDATION-PIPELINE
:justifies: REQ-DIAGNOSTIC-MODEL
:justifies: REQ-DIAGNOSTIC-OUTPUT

Humans, CI systems, editors, web interfaces, and agents all consume validation.
Stable structured diagnostics are therefore part of the product interface, not
incidental terminal strings.
:::

:::risk m_01KY7YA2CMCJSVJ3GVPQ3VTFYG
:id: RISK-DIAGNOSTIC-CASCADE
:title: One malformed item may cause misleading secondary errors
:status: open
:severity: high
:likelihood: medium
:affects: REQ-VALIDATION-PHASES
:affects: DES-VALIDATION-PIPELINE

Continuing after failures can create noisy cascades if dependent checks run on
an invalid partial model. The validation pipeline must distinguish an
independent issue from a check skipped due to a broken prerequisite.
:::

## Planned verification

:::test m_01KY7Y9R745TA9RJ916TT3J88R
:id: TEST-CHECK-PIPELINE
:title: Complete project check acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-CHECK-PIPELINE
:verifies: REQ-SCHEMA-CHECK-COMMAND
:verifies: REQ-VALIDATION-PHASES

A mixed fixture project shall contain independent valid and invalid files and
schema failures. Tests shall assert completed and skipped phases and continued
diagnostics from unaffected inputs.
:::

:::test m_01KY7Y9R75NJ7PYWJ1HP8ZGHPT
:id: TEST-DIAGNOSTICS
:title: Structured diagnostic contract test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-DIAGNOSTIC-MODEL
:verifies: REQ-DIAGNOSTIC-COLLECTION
:verifies: REQ-DIAGNOSTIC-ORDER
:verifies: REQ-DIAGNOSTIC-OUTPUT
:verifies: REQ-VALIDATION-EXIT-CODES
:verifies: REQ-VALIDATION-COMPATIBILITY

Golden human and JSON outputs shall assert stable codes, fields, related spans,
ordering, summary counts, warning escalation, and distinct process exit codes.
:::

:::test m_01KY7Y9R764CRC9B6TCZ5A28DQ
:id: TEST-ITEM-RELATION-VALIDATION
:title: Item and relation validation matrix test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-VALIDATE-ITEMS
:verifies: REQ-VALIDATE-RELATIONS

Table-driven fixtures shall cover every built-in constraint, field type,
reference failure, relation endpoint, cardinality, duplicate policy, and
external-target rule.
:::

:::test m_01KY7YA2CH54CW9G82S7E7083E
:id: TEST-GRAPH-RULES
:title: Configured graph rule evaluation test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-VALIDATE-RULES
:verifies: REQ-VALIDATE-CYCLES
:verifies: REQ-VALIDATE-ORPHANS

Fixtures shall compare schemas with different lifecycle, orphan, cardinality,
and acyclicity policies over the same item corpus and assert that only declared
rules produce issues.
:::
