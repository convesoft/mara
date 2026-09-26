# Traceability and evolution

Accepted product direction for 0.3.0, recorded after the POC comparison and
[external research](traceability-research.mara.md). These are planned outcomes,
not capabilities of the 0.2 release. Existing 0.2 syntax and interfaces remain
described in [format](format.mara.md) and [discovery](discovery.mara.md).

The workflow is to connect knowledge, declare what a project expects, and
explain missing or invalid obligations. It includes richer graph semantics,
current-state lifecycle and traceability rules, actionable diagnostics, and
traceability matrices. One project and its schema remain the evaluation boundary.
No mandatory engineering lifecycle, complete trace chain, or placeholder items
are imposed on projects that have not declared those expectations.

Requirements below own observable outcomes. The accepted
[relationship contracts](relations.mara.md) settle authoring and mutation;
[rule and matrix contracts](rules.mara.md) settle policy, diagnostics, matrix
output and its format boundaries. Remaining decisions at the end must be
resolved before implementing their affected contracts. CLI and MCP follow
[[REQ-SURFACE-PARITY]] throughout.

## Scenarios

:::mara scenario SCN-AUTHOR-TRACE-CONNECTION
:mid: 01M2FX4BQMS5A6W5KM4T0WJ7MD
:title: Author one relationship from either endpoint

An author connects a verification to a requirement from either item's context,
using the relation name appropriate to that endpoint. Another author expresses
the same relationship in supported inline syntax. Navigation shows one semantic
connection and lets the reader inspect each authored occurrence. Symmetric
associations can be authored from either endpoint without inventing an upstream
or downstream meaning.
:::

:::mara scenario SCN-CHECK-TRACE-OBLIGATIONS
:mid: 01M2FX4BQV6MJZRBSAGGEB0AP6
:title: Find unmet project obligations

A project owner requires approved requirements to have an approved verification,
accepted designs to reference a requirement, and mitigated risks to have incoming
mitigation. An author runs validation after ordinary Markdown edits. Mara names
the applicable rule, affected item and unmet condition, distinguishing a missing
link from linked items that do not qualify. A declared chain can reveal a gap
beyond the first hop. Fixing the actual knowledge makes the corresponding check
pass; unrelated links cannot conceal the gap.
:::

:::mara scenario SCN-READ-TRACE-VIEW
:mid: 01M2FX4BR2F15YVXCD89RTGDKQ
:title: Inspect trace coverage in a matrix

An author selects root items and a declared rule or request-local check for a
traceability matrix. A reader can inspect coverage states, relationship paths,
source evidence and unmet obligations. A requirement linked to an external
delivery ticket remains distinguishable from one with verification evidence.
Regenerating the matrix after source changes reflects current knowledge without
making its output another authoring authority.
:::

## Requirements

:::mara requirement REQ-INVERSE-RELATION-AUTHORING
:mid: 01M2FX4BR8X55V5KV8R00ZESY0
:title: Author directed relations using inverse aliases
:derives_from: SCN-AUTHOR-TRACE-CONNECTION

A schema can declare a canonical directed relation and an inverse alias.
Either endpoint can author the same relation using the appropriate name.
For example, `VER-A verifies REQ-B` and `REQ-B verified_by VER-A` describe
the same directed relationship when that alias is declared.

Resolve endpoint constraints after interpreting the alias. Schema inspection,
structured relation operations, and relation filtering must recognize declared
aliases consistently. Reject ambiguous declarations instead of guessing a name's
meaning. Every human-facing relationship view uses the name appropriate to
its displayed endpoint, including CLI navigation, matrices and diagnostics.
At an incoming endpoint, display the declared inverse alias without an incoming
prefix; use incoming plus the canonical name only when no inverse alias exists.
Structured results retain canonical relation and direction alongside the
endpoint-facing label.

Verify equivalent authoring and querying from both ends, rejection of reversed
invalid endpoint flavours, and one semantic count when both forms are present.
Verify endpoint-facing labels across these views with and without an inverse
alias, while structured canonical relation and direction remain unchanged.
:::

:::mara requirement REQ-SYMMETRIC-RELATIONS
:mid: 01M2FX4BRF8BSW2GRNTRRQ5B4A
:title: Represent symmetric associations without arbitrary direction
:derives_from: SCN-AUTHOR-TRACE-CONNECTION

A schema can declare a relation symmetric. Reversing its endpoints preserves
the same semantic association, and navigation presents it as symmetric rather
than choosing incoming or outgoing according to where it was authored.
Endpoint eligibility must be consistent when the endpoints are exchanged.

Verify that authoring the association at either or both ends produces one
semantic relationship and the same neighbours and counts. Directed relations
retain their direction; an inverse alias does not make a relation symmetric.
:::

:::mara requirement REQ-TYPED-INLINE-RELATIONS
:mid: 01M2FX4BRN24NBFKCBEPH120BM
:title: Express typed item relationships in Markdown bodies
:derives_from: SCN-AUTHOR-TRACE-CONNECTION

Support explicit typed inline references in item bodies using schema-defined
relation names and inverse aliases. They contribute the same semantic
relationships as metadata declarations. Bare mentions retain their existing
navigation-only meaning and do not satisfy a typed obligation.

Recognize typed references only in supported Markdown contexts, excluding code,
raw and escaped literal examples consistently with existing reference handling.
Retain each occurrence's source location. Rename, move, delete and relation
mutations must account for every supported authored form without leaving stale
references or silently removing unrelated prose.

Verify metadata/inline equivalence, literal examples, and reference-safe edits.
Inline spelling follows [[DES-RELATION-AUTHORING]]. Relationship removal
preserves surrounding prose and demotes inline assertions to ordinary navigation;
explicit occurrence removal reports whether other assertions still establish
the edge. The mutation contract and examples are in [[DES-RELATION-MUTATION]].
:::

:::mara requirement REQ-TRACE-COVERAGE
:mid: 01M2FX4BRV92A6G1GXX5DDAZ5P
:title: Count only relationships that satisfy the declared obligation
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

A coverage rule specifies relation kind, orientation and qualifying target
conditions. Support minimum/maximum qualifying counts and obligations applying
to every selected related item. Distinguish at least one qualifying target
from all targets qualifying.

For an approved requirement needing one approved verification, a draft
verification does not count; adding an approved verification can satisfy the
rule even while the draft remains, unless a separate all-targets rule forbids
it. Count a semantic relationship once regardless of authoring occurrences.
Neither an untyped mention nor an unrelated relation can satisfy the obligation.

Reports distinguish structural coverage, verification definitions and recorded
execution evidence. The existence of a code, test or ticket link never by
itself establishes implementation correctness or a successful test execution.
:::

:::mara requirement REQ-RELATION-CARDINALITY
:mid: 01M2FX4BS193R12Z27E3M7CQZQ
:title: Check declared relationship cardinality
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Allow a project to constrain the minimum and maximum number of semantic
relationships of a declared kind at an eligible endpoint. State which
orientation is counted. Repeated source occurrences and inverse spellings of
one relationship must not inflate that count; distinct relation kinds remain
distinct.

Verify a missing required relationship and an exceeded maximum, including
duplicates expressed from opposite ends. Conditional counts of qualifying
targets follow [[REQ-TRACE-COVERAGE]] rather than treating every existing link
as sufficient coverage.
:::

:::mara requirement REQ-RELATION-CYCLE-POLICY
:mid: 01M2FX4BS8BH26TSW1XWNWAWE4
:title: Check cycles only where the schema prohibits them
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Allow a project to prohibit cycles for a declared directed relation. Report
an offending cycle with the involved items and source occurrences. Resolve
aliases to canonical direction before checking, including a self-loop as a
cycle. Do not impose global acyclicity on unrelated relation types, mentions,
containment or symmetric associations.

Verify an alias-authored cycle, an ordinary acyclic chain, and a cycle in an
unconstrained relation that does not violate this policy. Rule traversal must
still terminate when the graph contains permitted cycles.
:::

:::mara requirement REQ-CURRENT-STATE-RULES
:mid: 01M2FX4BSGPX5K6KBMK9RQ1J1Y
:title: Validate project-defined item and lifecycle obligations
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Let a project declare named rules selecting items, applying conditions, and
checking obligations against their current fields or relationships. Lifecycle
status is an ordinary project-defined field; no universal status list or
workflow is built into the engine. Rules can express required fields under a
condition as well as status-dependent relationship obligations.

Evaluate the same rules after direct Markdown edits and structured authoring.
A rule whose selection or condition does not apply imposes no obligation on
that item. Invalid configuration or unresolved evaluation prerequisites must
not be reported as a successful check. Invalid corpus source, identity, field
or reference prerequisites skip policy evaluation, including item-targeted
checks; retain the original diagnostics and explicitly report unavailability.

Verify status-conditioned field and relationship checks, a non-applicable
draft item, and actionable diagnostics for invalid rule definitions. Previous
state and transition enforcement are reserved for 0.4.
:::

:::mara requirement REQ-BOUNDED-TRACE-CHAINS
:mid: 01M2FX4BSP29ZW778MW2EYV4XT
:title: Evaluate explicit relationship chains with bounded explanations
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Rules and traceability matrices may follow explicitly described relationship
steps, directions and target conditions. Each step retains its relationship
meaning and source provenance. Evaluate only the requested chain; unrelated
paths do not supply coverage. Distinguish immediate coverage from fulfillment of the
declared downstream obligations.

Traversal follows finite explicit shape/path depth; recursive shape references
and unbounded paths are rejected. Incomplete evaluation or truncated output
must be distinguishable from a complete result. No logical work counter or
partial execution prefix is required. Return the reported unmet obligation
without requiring enumeration of every distinct simple path or deepest leaf.
Result ordering and continuation are deterministic for unchanged inputs.

Verify a missing second-hop obligation, two distinct relation kinds between
the same endpoints, a cycle, and an explicit bound.
:::

:::mara requirement REQ-TRACE-DIAGNOSTICS
:mid: 01M2FX4BSX7T5C5GHF628WAHD1
:title: Expose stable diagnostics and project-defined rule severity
:derives_from: SCN-CHECK-TRACE-OBLIGATIONS

Validation provides stable machine-readable diagnostic codes, severity,
an actionable message, and the relevant item/source or configuration location.
Rule failures also identify the rule and reported unmet obligation, with
authored source and available relationship/count details. Composite failures
need not expose every nested result or the deepest failing leaf. Clients must
not parse prose to classify failures.

Project-defined rules support warning and error policies. Warnings remain
visible without making a project invalid; errors affect validity and CLI
failure status. Malformed source, unresolved identities and invalid rule
configuration remain distinguishable from policy failures. Define their
severity policy explicitly before implementing the diagnostic interface.

CLI and MCP share classifications and results. Preserve the whole-project
validation context and reporting-selection distinction of
[[REQ-PROJECT-VALIDATION]]. Verify one condition under warning and error
policies and a failure outside the selected diagnostic paths.
:::

:::mara requirement REQ-TYPED-EXTERNAL-TARGETS
:mid: 01M2FX4BT4VDC16M1TQWX1ZFPN
:title: Trace typed external references without importing lifecycle
:derives_from: SCN-READ-TRACE-VIEW

Allow schema-declared relation targets representing external references.
Distinguish them from internal item handles in authoring, inspection,
navigation and reports. Project rules can require an external reference of
the declared kind, including conditionally on a local item's status.

An external address has no Mara-managed fields, lifecycle or outgoing
knowledge graph. Its presence does not establish remote existence, remote
status, completed work or successful verification. Validation must not require
network access or credentials to evaluate the local reference obligation.

Verify a local approved item requiring a Linear ticket reference, an absent
reference, and a declaration that incorrectly tries to inspect remote status.
External synchronization and imported-state evaluation are later work.
:::

:::mara requirement REQ-TRACE-MATRIX
:mid: 01M2FX4BTAHMMPEEPX6JK418ZS
:title: Generate matrices that explain selected trace coverage
:derives_from: SCN-READ-TRACE-VIEW

Generate a traceability matrix from selected canonical items and declared
relationship steps or coverage obligations. Identify the selection and rule
scope, preserve relation meaning and source navigation, and show gaps and
non-qualifying targets rather than only successful links.

Distinguish an item outside a rule's applicability from an item that fails it.
Do not present a global coverage percentage whose denominator or obligation
is unspecified. Bounded output must identify incompleteness.

Verify an applicable covered item, an uncovered item, a non-applicable item
and a second-hop gap using the same semantics as validation.
:::

:::mara requirement REQ-SCHEMA-EVOLUTION
:mid: 01M2FX4BTSAJJSRHE0T75XWZER
:title: Preserve project knowledge through deliberate vocabulary changes
:depends_on: REQ-INVERSE-RELATION-AUTHORING

Provide a documented, deliberate migration workflow for the schema changes
introduced by 0.3 and demonstrated project vocabulary changes. Explain the
affected declarations, authored references and validation consequences before
applying edits. Preserve item MIDs, unrelated custom fields, prose and links;
do not reinitialize a customized project as a migration strategy.

Distinguish a label/alias change from changing relation direction, endpoint
eligibility or meaning. Do not silently reinterpret an existing relationship.
Any automated migration must provide reviewable proposed changes, recoverable
application and validation of the result.

Verify the workflow in [[DES-SCHEMA-MIGRATION-WORKFLOW]] on a customized schema
and corpus, including a relation vocabulary change and an invalid migration.
:::

## Settled design boundaries

:::mara design DES-CANONICAL-TRACE-RELATIONS
:mid: 01M2FX575WG9SP8EZJSTEE7VG9
:title: Normalize authoring forms while retaining their occurrences
:satisfies: REQ-INVERSE-RELATION-AUTHORING
:satisfies: REQ-SYMMETRIC-RELATIONS
:satisfies: REQ-TYPED-INLINE-RELATIONS
:satisfies: REQ-TYPED-EXTERNAL-TARGETS

Extend the disposable graph of [[ADR-PETGRAPH-DISCOVERY]] with a distinction
between semantic relationships and authored occurrences. Internal endpoint
identity is the MID. Canonical relation name and directed endpoints identify
a directed relationship; endpoint exchange does not create a second symmetric
relationship. Different relation kinds between the same endpoints remain
different relationships.

Resolve inverse aliases before endpoint validation, counting or traversal.
Metadata and typed inline occurrences can establish the same relationship.
Retain each occurrence's document, source span, spelling and authored endpoint
so diagnostics and edits can locate the actual source. Do not generate a
second stored assertion merely to provide reverse navigation.

Bare mentions and structural connections retain their own meaning. An external
target is explicitly distinguished from an internal item, without a fabricated
MID or lifecycle. Graph storage remains a derived in-memory projection.

External endpoint identity is its exact address under [[DES-RELATION-AUTHORING]].
A directed internal-to-external edge is identified by canonical relation,
source MID and external address. It has no fabricated external MID.

Authoring declarations and spelling follow [[DES-RELATION-AUTHORING]].
Duplicate add and whole-edge/occurrence removal follow [[DES-RELATION-MUTATION]];
results and bounded occurrence inspection follow [[DES-RELATION-INTERFACES]].
Format and client migration follow [[DES-RELATION-COMPATIBILITY]].
Reviewed schema and vocabulary migration follow [[DES-SCHEMA-MIGRATION-WORKFLOW]].
:::

:::mara design DES-DECLARATIVE-TRACE-RULES
:mid: 01M2FX5766P3HJZSVFKC4AGAZ8
:title: Use one declarative model for local and relationship obligations
:satisfies: REQ-CURRENT-STATE-RULES
:satisfies: REQ-TRACE-COVERAGE
:satisfies: REQ-BOUNDED-TRACE-CHAINS

The rule model separates identity, item selection, applicability conditions,
obligations and failure policy. Local obligations inspect current item fields;
relationship obligations select canonical relation kinds and orientation, then
check qualifying endpoints or an explicit chain. Lifecycle checks use the same
model as other project policy.

Expose enough explanation to distinguish non-applicability, a satisfied
obligation, an unmet obligation and unavailable evaluation caused by invalid
prerequisites or bounds. A partial evaluation is never a successful full check.
Matrices and validation reuse these semantics.

The YAML authoring profile, generated SHACL bindings and worked examples are
in [[DES-TRACE-RULE-GRAMMAR]].
Structural graph policies compose under [[DES-TRACE-GRAPH-CONSTRAINTS]].
Bounds and diagnostics follow [[DES-TRACE-DIAGNOSTIC-INTERFACE]]; matrices follow
[[DES-TRACE-VIEW-INTERFACES]]. Compatibility, including the future transition
exercise, follows [[DES-TRACE-CONTRACT-COMPATIBILITY]].
Keep syntax declarative and project-owned; no arbitrary code execution or
general workflow engine is introduced.
:::

:::mara decision ADR-CURRENT-STATE-BEFORE-TRANSITIONS
:mid: 01M2FX576DBS55CVWJ2H5GESZQ
:title: Introduce current-state policy before transition validation
:justifies: REQ-CURRENT-STATE-RULES
:justifies: DES-DECLARATIVE-TRACE-RULES

Implement current-state lifecycle and traceability rules in 0.3. Add
transition checks in 0.4 once item/document comparison across revisions can
supply previous and current states. A status field alone does not establish
that a transition was allowed or reviewed.

Design the 0.3 rule grammar so a later explicit transition context can be added
without changing the meaning of existing current-state rules. Before accepting
that grammar, demonstrate both a current-state example and a future
previous/current-state example; the latter is a compatibility design exercise,
not a 0.3 executable rule.

This keeps direct Markdown editing supported and avoids inventing a mutation
history authority ahead of the accepted change-review workflow. Suspect-link
acknowledgement and evidence freshness policy remain 0.4 design work.
:::

## Verification

:::mara verification VER-TRACEABILITY-WORKFLOW
:mid: 01M2FX576MTSZ2DSVHMPRG2AC7
:title: Verify traceability through real authoring and inspection

On a temporary project, use a project-owned schema to create requirements,
designs, verification and risk items with optional statuses. Exercise the
scenarios in [[SCN-AUTHOR-TRACE-CONNECTION]], [[SCN-CHECK-TRACE-OBLIGATIONS]] and
[[SCN-READ-TRACE-VIEW]] through CLI and MCP, including direct Markdown edits.

Use the same fixtures to verify alias/inline normalization, symmetric links,
declared cardinality and cycle checks, current-state obligations, a second-hop
gap, external references and traceability matrices. Inspect diagnostics and source
navigation before and after correcting each gap. Exercise warning/error policy
and bounded output without mistaking incomplete evaluation for a pass.

Run the supported migration workflow on a customized copy and compare MIDs,
unrelated content and links before/after. Keep code-pilot verification with its
eventual language/workflow design.

For relationship authoring, use the declarations in [[DES-RELATION-AUTHORING]]
and requests in [[DES-RELATION-INTERFACES]]. Establish one edge through metadata,
inverse metadata and inline source; inspect three occurrences and one semantic
edge. Check directed and symmetric self-edges with omitted direction and each
explicit direction filter against [[DES-RELATION-INTERFACES]], including a
one-connection page limit and continuation without repeated self-edges.
Reject a duplicate add without changing bytes. Run both removal examples
in [[DES-RELATION-MUTATION]] from the same initial fixture and compare exact
source text, counts and edge presence on CLI and MCP. Repeat with a symmetric
pair and an external address. Check stale occurrence rejection, alias collision,
invalid endpoint flavours, literal typed examples, rename and a move/removal
blocked by a surviving Markdown anchor link. Exercise occurrence continuation
with more than 20 assertions, and the customized migration examples in
[[DES-RELATION-COMPATIBILITY]].

For rules and views, use [[DES-TRACE-RULE-GRAMMAR]]'s fixture table and nested
evidence check through validation and matrix requests. Repeat the uncovered
approved requirement with warning severity, with its path hidden, and with
invalid corpus prerequisites. Assert state, counts, validity, diagnostic code and
source/configuration locations against [[DES-TRACE-DIAGNOSTIC-INTERFACE]].
Test empty every with/without minimum, missing versus blank fields, a passing
SHACL alternative with a failing alternative, and an unavailable prerequisite.

Verify structural and conditional count failures independently under
[[DES-TRACE-GRAPH-CONSTRAINTS]], including an alias-authored cycle, a self-loop
and a permitted cycle. Evaluate an 8-step chain and reject a 9-step definition;
output continuation must not resume evaluation or alter validity. Follow output
cursors to completion and reject them after a source or request-option change.

Use [[DES-TRACE-CONTRACT-COMPATIBILITY]] to inspect the customized schema
migration and future transition exercise; the future syntax must be rejected
by 0.3.

Load the documented YAML examples from explicit configured rule files without
prefix/context declarations or intermediate Turtle.
Verify project-format/binding-version errors, missing files, malformed YAML,
unsupported executable predicates and an unknown named shape. Confirm typed
field projection, absent versus empty values, and RDF deduplication of repeated
values. Exercise the numeric example in [[DES-TRACE-RULE-GRAMMAR]] with
score 1 and 1.0: a typed double condition must apply, so a missing owner fails
and a present owner passes. Check typed hasValue/in matches and nonmatches,
plain integral literals retaining integer semantics, and malformed typed
literals rejected before evaluation. Exercise condition-shape applicability
for approved, draft, absent and invalid status; only the first evaluates obligations, while invalid source is
unavailable and skips policy evaluation across the corpus. An encountered
nested engine error must prevent a full pass even if upstream folds it into
nonconformance. Preserve reported obligation YAML pointers and source spans,
including escaped strings, aliases and generated blank-node shapes. Unknown
constraint keys, duplicate YAML keys, context overrides and unsupported targets
must fail before conversion. Verify schema-derived bindings with a new flavour
and a custom field named class, literal values equal to vocabulary names,
explicit field/schema qualification, and SHACL and/or/in RDF lists. Change
the schema or a rule source between pages and reject the old cursor. Exercise
YAML request-check files through CLI/MCP without enabling them as project policy.
Reject Turtle/JSON-LD source files. Verify native pattern matching without a
logical budget, stable completed diagnostic pages, and absence of max_work/work
from the CLI/MCP contract. Check that Cargo resolves SHACL from the registry
without a source patch. Exhaustive internal traces are not acceptance criteria.
:::

## Code-traceability pilot

0.3 includes evaluating a pilot for one demonstrated language and workflow,
not a commitment to general multi-language extraction or test-report imports.
Select the language and a real requirement-to-code/test example before
scheduling its implementation. Compare source markers and document-authored
references, define supported file/symbol boundaries and diagnostics, and verify
source navigation and behavior after code changes. Record the result and
limitations here before making a shipping commitment. A source association
alone cannot establish execution evidence under [[REQ-TRACE-COVERAGE]].

### MARA-69 evaluation

The pilot uses Rust and the real `REQ-RELATION-CARDINALITY` workflow:
`src/graph_constraints.rs::evaluate` implements the check, and
`tests/cli.rs::structural_relation_policies_validate_normalized_edges_through_cli_and_mcp`
defines a verification. The tested boundary is project-relative Rust files and
top-level free functions; file-only targets are also supported. The disposable
resolver in `experiments/mara-69/` parses Rust with `syn` and uses Rust doc
comments as attached markers:

Run `cargo run --manifest-path experiments/mara-69/Cargo.toml -- .
code:src/graph_constraints.rs::evaluate` from the repository root to resolve
the unchanged source.

```rust
/// @mara implements REQ-RELATION-CARDINALITY
pub(crate) fn evaluate(/* ... */) { /* ... */ }
```

The corresponding document-authored candidate is
`:implemented_by: code:src/graph_constraints.rs::evaluate`, where
`implemented_by` would be the declared inverse of `implements`. The test marker
uses `@mara verifies REQ-RELATION-CARDINALITY`. Relation names and endpoint
permissions must come from the project schema; these example relations are not
declared for code endpoints in the current self-hosting schema. The candidate
target notation is `code:<project-relative-path>[::<language-native-symbol>]`.

The experiment read copies of the actual source files with the two doc comments
inserted. It derived a backlink and comment location from each marker and
resolved the corresponding `code:` selector to the function name location.
`code:src/graph_constraints.rs` resolved to the file. Renaming `evaluate` in
the copy changed the marker-derived backlink to `::evaluate_renamed`; the old
document-authored selector reported a missing symbol. A missing file reported
a missing file, and two same-named definitions reported an ambiguous selector.
These are prototype outputs, not Mara validation diagnostics or editor links.

The pilot does not parse Mara item metadata, validate marker relations against
the schema, add graph edges, or expose backlinks through CLI/MCP. Its Rust
resolver does not handle methods, nested modules, trait items, macros, ordinary
`//` comments, or other languages. The ambiguous-definition fixture is
syntactically parseable but not valid compiled Rust. No source association
establishes a passing test result.

**Recommendation:** defer production code links from 0.3. Keep this evaluation
as evidence for a follow-up contract covering typed code endpoints, schema
declarations, marker attachment, native symbol resolution, diagnostics,
navigation, and an adapter boundary proven with another language.

Optional engineering-template lifecycle examples may demonstrate these rules;
do not silently add required statuses or policy to existing projects. Template
defaults need review alongside the rule examples.

For 0.4, retain Git comparison, immutable-identity history, review candidates,
suspect-link acknowledgement and transition checks. Their baseline and evidence
freshness semantics are not established by the 0.3 current-state checks.
