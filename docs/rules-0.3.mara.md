# Rules, diagnostics and trace views in 0.3

Contracts for the planned 0.3 implementation, extending
[traceability](traceability.mara.md) and the accepted
[relationship contracts](relations-0.3.mara.md). Examples describe intended
results, not checks executed by the 0.2 binary. The active schema and executable
remain unchanged. Project examples require the illustrated vocabulary; they
do not add lifecycle policy to bundled templates or existing projects.

:::mara design DES-TRACE-RULE-GRAMMAR
:mid: 01M2JNZMJ0VT20GWH4HF6DBBC5
:title: Declare current-state predicates and qualified relationship obligations
:satisfies: REQ-CURRENT-STATE-RULES
:satisfies: REQ-TRACE-COVERAGE
:satisfies: REQ-BOUNDED-TRACE-CHAINS

Schema format 3 adds an optional top-level `rules` mapping, defaulting to
empty. Each key is a unique project-owned rule name using lowercase snake case;
it is stable configuration identity, not an item ID or MID. A rule contains
`select`, optional `when`, required `require`, and optional `severity`
(`error` by default, or `warning`). Its context is always the current snapshot.
Reject unknown keys/operators and duplicate YAML mapping keys.

## Selection and applicability

`select` requires a nonempty `flavours` list of declared flavours. Optional
`paths` selects document paths/subtrees using the existing retrieval path
grammar. Omitted filters add no restriction; supplied lists must be nonempty. OR within a category, AND
between categories. Unknown vocabulary is a configuration error;
an empty result from valid filters is not an error. Persisted rules do not
select mutable item IDs; exact ID/MID selection remains a view-request filter.

`when` is a local predicate, defaulting to true. It cannot traverse relations.
Selection mismatch or false applicability means `not_applicable`, imposing
no obligation. A missing optional status therefore does not activate a rule
whose condition is `status == approved`. Make status universally required in
the flavour declaration if that is the project's intention.

## Predicate grammar

A predicate is exactly one of the following mappings. `when` permits only
field predicates and local `all`/`any` compositions; `require`, `qualifies`
and `every` also permit `related`.

| Shape | Meaning |
|---|---|
| `{field: NAME, exists: true}` | At least one authored value is present. |
| `{field: NAME, exists: false}` | No authored value is present. |
| `{field: NAME, equals: VALUE}` | At least one present value equals VALUE. |
| `{field: NAME, in: [VALUE, ...]}` | At least one present value equals a member of the nonempty list. |
| `{field: NAME, nonblank: true}` | At least one present string value has a non-whitespace character. |
| `{all: [PREDICATE, ...]}` | Every predicate passes. |
| `{any: [PREDICATE, ...]}` | At least one predicate passes. |
| `{related: RELATION_CHECK}` | Evaluate the selected relationships as below. |

Lists for `all` and `any` must be nonempty. A field predicate has exactly
one operator. Field names refer only to schema-declared custom fields, not
title, ID, MID or relations. Each field must be declared on every possible
internal flavour in that predicate's scope. Narrow `select.flavours` or
`target.flavours` when needed. Wrong literal types or enum values are
configuration errors, even when no item currently matches.

Compare parsed schema scalar types: booleans and numbers by value, strings
and enums exactly and case-sensitively after ordinary metadata trimming.
Do not coerce strings to numbers. `nonblank` is allowed only for string fields.
For repeatable fields, the operators above test existence or any qualifying
value; they do not require every repeated value to match. A missing field
fails `equals`, `in` and `nonblank`; a present empty string passes
`exists:true` and `equals:""`, but fails `nonblank:true`.
An invalid authored value is an unavailable prerequisite, not a missing value.
No negation, regex, arithmetic, scripts, interpolation or user functions are
part of this grammar.

## Relationship checks and chains

`RELATION_CHECK` contains:

| Key | Contract |
|---|---|
| `relation` | Required one schema relation name or inverse alias. Builtins are not policy edges. |
| `direction` | Required `outgoing`, `incoming` or `symmetric`, relative to the item and canonical relation. |
| `target` | Required selector with `kind: item` or `kind: external`; item selectors may add nonempty `flavours`. |
| `qualifies` | Optional endpoint predicate, default true. |
| `count` | Optional mapping with `minimum` and/or `maximum` nonnegative integers; minimum cannot exceed maximum. |
| `every` | Optional predicate that must pass for every selected endpoint. |

At least one of `count` and `every` is required. An omitted count bound is
unconstrained. Resolve aliases without reversing the explicit direction,
matching relationship navigation. Require symmetric direction exactly for a
symmetric declaration. Reject impossible orientation/target-kind/flavour
combinations against the declaration. External targets are outgoing only;
`target.kind:external` prohibits `qualifies`, `every`, fields and further
steps. External checks can count explicit addresses only.

First select semantic edges by relation, orientation and target selector.
Count the selected edges whose endpoint satisfies `qualifies`, including
one per edge even when several authored occurrences assert it. `every`
examines all selected edges, including those rejected by `qualifies`.
A flavour/kind excluded by `target` is outside this obligation, not a
non-qualifying target. Counts and `every` must both pass when both are present.

An empty selected set has qualifying count zero and satisfies `every`.
Require `count: {minimum: 1}` as well when presence is mandatory.
For one relation at one item each distinct endpoint contributes one semantic
edge. Directed and symmetric self-edges count once in their selected direction.
Different relation kinds never substitute for the named kind.

Nested `related` predicates in `qualifies` or `every` form explicit chains.
Each hop is evaluated at the previous endpoint; a parent count counts its
immediate qualifying edges, not all downstream paths or terminal endpoints.
For example, a requirement's two verifications with the same evidence still
count as two first-hop verification edges. Nesting preserves whether downstream
coverage is required for some or every first-hop verification.
Never expand unspecified relations or recursively apply other named rules.

## Evaluation states

Report `not_applicable`, `passed`, `failed` or `unavailable` for each
item/rule pair. Applicable obligations expose the same states except
`not_applicable`. Evaluate independently discoverable obligations even when
a sibling fails. If any prerequisite/child evaluation is unavailable, the
containing predicate and applicable rule are unavailable, including `any`
with another passing child; retain known failures in its explanation.
Otherwise use ordinary all/any logic.

Invalid source, unresolved identities, invalid rule definitions or exhausted
evaluation bounds must never become zero counts or false applicability.
Continue independent checks whose prerequisites remain available. Policy
severity changes reporting/validity, not selection or predicate truth.
Ordinary source edits and structured mutations are evaluated identically;
policy failures do not create a new mutation gate.

## Worked current-state rules

These schema excerpts assume optional enum statuses with the illustrated
values, an optional string `owner` on requirements, and directed relations
`verifies: verification -> requirement`, `satisfies: design -> requirement`,
`mitigates: design -> risk`, and `evidences: evidence -> verification`.

```yaml
rules:
  approved_requirement:
    select: {flavours: [requirement]}
    when: {field: status, equals: approved}
    require:
      all:
        - {field: owner, nonblank: true}
        - related:
            relation: verifies
            direction: incoming
            target: {kind: item, flavours: [verification]}
            qualifies: {field: status, equals: approved}
            count: {minimum: 1}
  accepted_design:
    select: {flavours: [design]}
    when: {field: status, equals: accepted}
    require:
      related:
        relation: satisfies
        direction: outgoing
        target: {kind: item, flavours: [requirement]}
        count: {minimum: 1}
  mitigated_risk:
    select: {flavours: [risk]}
    when: {field: status, equals: mitigated}
    require:
      related:
        relation: mitigates
        direction: incoming
        target: {kind: item, flavours: [design]}
        count: {minimum: 1}
```

| Fixture | Expected rule result |
|---|---|
| Approved requirement, owner present, only a draft verification | Failed: qualifying count 0, selected count 1. |
| Add an approved verification; retain the draft and repeat the approved edge inline | Passed: qualifying count 1, selected count 2. |
| Add `every: {field: status, equals: approved}` to that relationship check | Failed: the draft violates every; count still passes. |
| No verification, with only the every check and no count | Passed: every over an empty set. |
| No verification, with every plus minimum 1 | Failed: minimum count, not every. |
| Approved requirement with blank/missing owner | Failed: local nonblank obligation. |
| Draft requirement or requirement without optional status | Not applicable; no coverage obligation. |
| Accepted design without a satisfies edge | Failed; add one requirement edge to pass. |
| Mitigated risk without incoming mitigation | Failed; add one design mitigation to pass. |

To require at least one approved verification with recorded passing evidence,
replace the first example's `qualifies` predicate with:

```yaml
all:
  - {field: status, equals: approved}
  - related:
      relation: evidences
      direction: incoming
      target: {kind: item, flavours: [evidence]}
      qualifies: {field: outcome, equals: passed}
      count: {minimum: 1}
```

Declare evidence's optional enum `outcome` with `passed` among its values.
An approved verification without evidence fails the nested minimum, so it
does not qualify at the first hop. Adding a passing evidence item fixes that
gap. A draft verification with passing evidence still fails the status check.
This checks recorded assertions; it does not execute a verification or prove
that the evidence is trustworthy or fresh.
:::

:::mara design DES-TRACE-GRAPH-CONSTRAINTS
:mid: 01M2JP04Z4R4RJP4MWYNN442Z4
:title: Compose structural graph policies with conditional rules
:satisfies: REQ-RELATION-CARDINALITY
:satisfies: REQ-RELATION-CYCLE-POLICY

Schema format 3 extends each relation declaration with optional
`cardinality` and `acyclic`. These are structural policies over semantic
edges, independent of conditional rules in [[DES-TRACE-RULE-GRAMMAR]].

```yaml
relations:
  satisfies:
    description: The design provides a solution for the requirement.
    source: [design]
    target: [requirement]
    cardinality:
      outgoing: {minimum: 1, maximum: 3, severity: error}
  depends_on:
    description: The source depends on the target.
    source: [requirement]
    target: [requirement]
    acyclic: {severity: warning}
```

`cardinality` maps eligible directions to mappings containing `minimum`
and/or `maximum`, with optional `severity` defaulting to error. Count bounds
follow the rule grammar; require at least one bound. Directed declarations
permit incoming/outgoing; symmetric declarations permit symmetric only.
External-capable relations can constrain outgoing counts, including internal
and external targets. Incoming counts apply only to declared internal target
flavours, never external addresses. Evaluate minimums even at eligible items
with zero incident edges. A self-edge counts once for each separately requested
orientation. Unknown keys and incompatible directions are schema errors.

Omitting a direction imposes no count constraint. Omitting `acyclic` allows
cycles. Present `acyclic` is a mapping with optional severity, default error;
it prohibits cycles in that one canonical directed relation. Reject it on
symmetric relations. External addresses are terminal and cannot close a cycle.
Aliases and repeated assertions normalize before either policy runs.

For each constrained relation, report one deterministic witness cycle per
cyclic strongly connected component, including a singleton with a self-loop.
Sort members by MID, start at the smallest member, and choose the first simple
cycle from depth-first traversal of outgoing neighbours in MID order.
List its ordered canonical edges and source inspection references. This is
evidence of the component's violation, not enumeration of every cycle.
A permitted cycle in another relation supplies no diagnostic.

A cardinality or cycle violation uses its configured warning/error severity.
Invalid declarations or unavailable graph evaluation remain errors regardless
of that policy. Conditional rules and structural constraints are conjunctive:
passing one never waives the other. Report failures under both identities
when both are independently unmet; do not silently merge their meanings.

Example: a design with four satisfies edges violates maximum 3 even if its
accepted-design rule needs only one and passes. A conditional approved-target
count ignores draft targets; structural cardinality counts them. Duplicate
metadata/inline/alias assertions of one edge change neither count.
Cycle prohibition does not change finite explicit-chain semantics.
:::

:::mara design DES-TRACE-DIAGNOSTIC-INTERFACE
:mid: 01M2JP1YNBM7T7G9E67CV0RP6V
:title: Bound evaluation and expose stable validation diagnostics
:satisfies: REQ-TRACE-DIAGNOSTICS
:satisfies: REQ-BOUNDED-TRACE-CHAINS
:satisfies: REQ-PROJECT-VALIDATION
:satisfies: REQ-SURFACE-PARITY

Validation and trace evaluation share these limits and diagnostic semantics.
They do not change relationship mutation errors in [[DES-RELATION-INTERFACES]].

## Work, output and continuation

Accept `max_work` (CLI `--max-work`), integer 1–1,000,000, default 100,000.
One logical work unit is charged for each item/selection test, predicate
invocation at an item, semantic-edge examination by a relationship check,
and vertex/edge visit by a graph-constraint pass. Schema validation charges
one unit per visited declaration, field constraint or rule predicate node;
specification generation charges one per emitted source node/edge before
content fragmentation. Charge logical visits even when cached; count duplicate assertions only during normalization, not as
extra semantic edges. Sort items by path/start byte, rules by name, child
predicates in authored order, and edges by canonical relation and endpoint
identity before evaluation. This fixes the evaluated prefix independently of
hash iteration or memoization. Loading/parsing source is not covered by this
evaluation budget and must retain existing read/error behavior.

Allow at most eight nested relationship steps and predicate depth 32
(count the root as depth 1, including all/any and related endpoint predicates).
Reject deeper definitions as invalid configuration, not partially valid rules.
Finite nesting terminates even when unconstrained graphs contain cycles;
graph cycle checks visit a finite normalized graph, not arbitrary paths.

Before a work unit would exceed the limit, stop evaluation and report
`evaluation_limit` with limit and used units, set `evaluation_complete:false` and `valid:false`. Unvisited
checks are unavailable, never passed. Invalid prerequisites similarly make
affected checks unavailable and prevent a full pass; retain independent
diagnostics. A finite failed policy check is complete, not unavailable.
The evaluation limit itself is always an error, even for warning-only rules.

Output pagination is separate from evaluation. All page-based interfaces use
`limit` 1–100 (default 20) and a 65,536-byte serialized domain-response budget,
including envelope and cursor, excluding transport framing. `has_more` and
`next_cursor` describe remaining output of this evaluation, not remaining
evaluation work. Repeat unchanged inputs and limits with the cursor; reject
schema/corpus/project/options changes as `stale_cursor`. Re-evaluation may
reconstruct the same deterministic result; a server need not persist a job.

A higher `max_work` requires a fresh request without the old cursor.
At the maximum, report the unresolved limit; do not suggest an output-path
filter as a way to validate less of the project. Output bounds never silently
drop a record. Split long content as specified by the view contract; if an
indivisible identity/location or diagnostic cannot fit alone, return
`output_limit` naming the source/configuration to shorten.

## Diagnostic vocabulary and severity

Diagnostic `code` values classify failures at creation, never by parsing
message strings. These are stable categories; detail fields refine them.

| Code | Meaning and severity |
|---|---|
| `project_invalid` | Invalid project configuration; error. |
| `schema_invalid` | Invalid schema declaration/type; error. |
| `format_unsupported` | Unsupported persisted format; error with migration guidance. |
| `source_invalid` | Malformed Markdown/item/metadata structure; error. |
| `identity_invalid` | Missing, malformed or duplicate human ID/MID, or wrong prefix; error. |
| `field_invalid` | Missing required field/body/title, invalid value, undeclared field or forbidden repetition; error. |
| `reference_unresolved` | Unresolved or ambiguous internal item/Markdown reference; error. |
| `relation_invalid` | Invalid authored relation, orientation or endpoint; error. |
| `rule_invalid` | Invalid rule definition, operator or scope; error. |
| `rule_failed` | Applicable current-state rule failed; rule's severity. |
| `relation_cardinality` | Structural minimum/maximum failed; constraint's severity. |
| `relation_cycle` | Prohibited cycle component; constraint's severity. |
| `evaluation_unavailable` | A required check lacks valid prerequisites; error. |
| `evaluation_limit` | Work budget exhausted; error. |

For invalid rule vocabulary use `rule_invalid`, rather than also reporting
`schema_invalid` for the same defect. Invalid rule values in authored items
retain source/field codes. For overlaps report the most specific applicable
category, with independent defects reported separately.
Diagnostic messages are actionable prose, not stable identifiers.
No severity overrides can downgrade structural/configuration failures.
Warnings are visible but do not invalidate an otherwise complete result.

Each diagnostic contains `code`, `severity`, `scope`
(project/schema/document/item), `message`, and a `location`. Location
contains project-relative `path` and optional one-based `line`, UTF-8
`start_byte`/`end_byte` and JSON Pointer `pointer` for configuration.
Only available coordinates are populated; never invent line numbers.
Retain legacy `path` and `line` fields as aliases of location coordinates.
An external configured schema path remains absolute.

Item diagnostics add `item:{id,mid}` when unambiguous; configuration/rule
diagnostics add `rule` when known. Rule failures add `obligation`, the
JSON Pointer within the rule, and `details` identifying
`kind:field|minimum|maximum|every|all|any`. Count details include
`selected_count`, `qualifying_count` and the violated bound.
Relationship explanations retain canonical relation, direction and
endpoint-facing label, plus item/edge references. Locations of all assertions
remain inspectable via relation get, not an unbounded inline list.

Emit one `rule_failed` per failed item/rule pair, pointing to the first
unsatisfied leaf in predicate order that contributes to the root failure.
Do not emit failures for unsuccessful alternatives of a passing any.
The matrix exposes the remaining check results, including all failed
alternatives when any fails. Unavailable rules produce
`evaluation_unavailable` referencing their prerequisite diagnostics instead
of a fabricated policy failure. An exhausted request emits one global
`evaluation_limit`, not one diagnostic for every unvisited item.

## Validation response and entry points

CLI JSON and MCP return validation `format_version:1` with
`project`, `target`, `valid`, `evaluation_complete`, `work:{used,limit}`,
`diagnostics`, `summary`, `selection`, `has_more` and `next_cursor`.
`target.kind` is project/item/schema; item targets retain `id`.
Schema results retain `path`, `flavours` and `relations` when the schema is
available; counts are null if it cannot be loaded.
`summary` contains `errors`, `warnings` and `counts_exact`.
Counts describe the entire validation target before reporting selection and
pagination. They are lower bounds with `counts_exact:false` if prerequisites
or a work bound prevent complete evaluation.

`valid` is true only with complete evaluation and zero errors. A page with no
diagnostics can therefore have `valid:false`. `selection.paths` and
`selection.omitted_diagnostics` preserve the existing path-reporting contract;
the latter counts produced diagnostics hidden by paths, not unseen checks or
records deferred to later pages. Project/schema diagnostics remain visible.
Item validation checks the selected item in full corpus context, including
its incident constraints and cycles, with prerequisite errors that affect it.
Schema validation checks configuration, rule typing and graph declarations,
not item predicates.

Sort diagnostics by scope (project, schema, document, item), path, start byte
(or line when byte is absent; missing coordinates first), item MID, rule,
obligation pointer and code, with message as final deterministic tie-breaker.
No page boundary changes summary, validity or evaluation completeness.

| CLI | MCP |
|---|---|
| `project validate --path docs/ --limit 20 --max-work 100000` | `project_validate {paths:["docs/"], limit:20, max_work:100000}` |
| `item validate REQ-A --limit 20` | `item_validate {id:"REQ-A", limit:20}` |
| `schema validate --limit 20` | `schema_validate {limit:20}` |

All three accept cursor and max_work; only project validate accepts reporting
paths. Project selection follows existing CLI/MCP conventions. CLI
`--format json` returns the domain result; default text renders it.
CLI exits 0 for valid, 1 for an invalid/incomplete validation result;
MCP returns a successful tool result containing `valid:false` for that same
completed operation. Invalid arguments, stale cursors, I/O failure preventing
a result, or an oversized indivisible record are operation errors:
`{format_version:1,error:{code,message}}`, CLI nonzero and MCP isError true.
Operation codes are `invalid_argument`, `stale_cursor`, `io_error` and
`output_limit`; do not confuse these with policy diagnostics.

For one uncovered approved requirement and no other errors, changing the rule
from error to warning changes summary errors/warnings from 1/0 to 0/1,
valid false to true, and CLI exit 1 to 0; its matrix state remains failed.
If its document is outside selected reporting paths, the diagnostic is omitted
but those totals, validity and exit status remain unchanged.
:::

:::mara design DES-TRACE-VIEW-INTERFACES
:mid: 01M2JP3PJV8WZKNR1WWF0GJMS4
:title: Generate bounded matrices and specifications from explicit selections
:satisfies: REQ-TRACE-MATRIX
:satisfies: REQ-GENERATED-SPECIFICATION
:satisfies: REQ-BOUNDED-TRACE-CHAINS
:satisfies: REQ-SURFACE-PARITY

Trace views are disposable, read-only projections of one loaded project.
They share rule semantics with [[DES-TRACE-RULE-GRAMMAR]], graph identity with
[[DES-CANONICAL-TRACE-RELATIONS]], and work/output bounds with
[[DES-TRACE-DIAGNOSTIC-INTERFACE]]. Do not create saved view definitions or
modify canonical files. Each request carries its selection.

## Selection and public operations

Both operations accept optional nonempty `ids`, `flavours`, `fields`
and `paths`, following exact item-retrieval filter semantics and the existing
path/subtree grammar. Require at least one filter or explicit `all:true`;
all cannot be combined with filters. Reject unknown vocabulary/IDs and
invalid paths; a valid selection matching nothing returns an empty view.
Fields use existing `[{key,value}]` scalar-text equality filters.
Selection chooses roots only; related items outside it remain available
for evaluation and are identified as outside the root selection.

| CLI after `mara` | MCP |
|---|---|
| `trace matrix --flavour requirement --rule approved_requirement` | `trace_matrix {flavours:["requirement"], rules:["approved_requirement"]}` |
| `trace matrix --id REQ-A --check '{"related":{"relation":"verifies","direction":"incoming","target":{"kind":"item"},"count":{"minimum":1}}}'` | `trace_matrix {ids:["REQ-A"], check:{related:{relation:"verifies",direction:"incoming",target:{kind:"item"},count:{minimum:1}}}}` |
| `trace specification --path docs/` | `trace_specification {paths:["docs/"]}` |
| `trace specification --flavour requirement --field status=approved` | `trace_specification {flavours:["requirement"], fields:[{key:"status",value:"approved"}]}` |

Both accept `--all`, repeatable `--id`, `--flavour`, `--field`,
`--path`, and `--limit`, `--cursor`, `--max-work`.
Matrix additionally requires either repeatable `--rule` / nonempty `rules`,
or one `--check` / `check`, never both. A check is one relationship predicate
using the rule grammar, optionally with explicit nested steps. It applies
unconditionally to roots; validate its typing against every selected flavour.
It is a request-local observation, not a new project validation policy.
Named rules retain their own selection and applicability, intersected with
view roots; unknown names are errors. Distinguish request checks from schema
rule names in output; they cannot shadow a persisted rule.

CLI default text renders Markdown for these two commands only;
`--format json` returns the structured result. MCP returns the same JSON
domain result and, when `render:"markdown"` is requested, its Markdown
rendering in a `markdown` field. This field counts against the page budget.
CLI text uses the same page construction as MCP render markdown, including
its combined JSON/Markdown budget; CLI JSON matches MCP without render.
Changing render mode is a changed request and requires a fresh cursor.
No CSV, HTML or PDF contract is introduced.

A view has `format_version:1`, `kind:matrix|specification`, normalized
`selection`, `evaluation_complete`, `work`, `records`, `has_more` and
`next_cursor`. Records of kind `issue` use validation diagnostic shapes for
problems preventing a complete view. It does not claim whole-project validity.
Known policy failures are data in a complete matrix: CLI exits 0 and MCP
isError false. Unavailable evaluation gives CLI exit 1 and MCP a domain result
with evaluation_complete false; operation errors follow the validation family.

## Matrix records and explanations

Return a stream of records, so a high-degree item or long explanation cannot
force an unbounded nested row:

| Record kind | Required meaning |
|---|---|
| `result` | `root` item descriptor, `evaluation` rule/check identity, and overall `state`. |
| `check` | `reference`, `root`, `evaluation`, `obligation` predicate JSON Pointer, `context`, `parent` check reference (null at root), `state`, and `condition` predicate; relationship checks add `counts` and `every`. |
| `edge` | `check` reference, canonical `edge`, endpoint-facing `label`, `direction`, `endpoint`, `qualification`, `every`, and `occurrence_count`. |
| `issue` | `diagnostic` preventing complete evaluation. |

`evaluation` is `{kind:"rule",name:"approved_requirement"}` or
`{kind:"check"}`. `root` and item endpoints use discovery item descriptors;
external endpoints use the relationship contract's external descriptor.
Every record carries `kind`. `condition` contains only the local operator and
its scalar parameters; nested predicates have their own check records.
A check's `counts` contains `selected`,
`qualifying`, `minimum` and `maximum`; omitted bounds and unavailable totals
are null. `every` and `qualification` use the predicate states or null when
that test is not requested. `context` is an ordered array of canonical edges.

An endpoint context is the root plus the ordered canonical edges to that
check; at most eight hops. `check` references are snapshot-bound opaque
identifiers, not durable item identities. They connect records across pages.
Counts belong to the immediate check: include selected and qualifying totals,
declared minimum/maximum, and every state where present. Counts are null
when unavailable, not misleading zeros. Leaf field checks name the field
and expected operator/value; source navigation supplies full authored values.

Emit one result for each root/rule pair, including not-applicable roots.
Only applicable rules have check records. Emit check records in predicate
preorder; relationship edge records follow their owning check in canonical
endpoint order, each followed by its nested checks. Emit failed alternatives
for explanation even when the parent any passes; distinguish child state
from root state. Never enumerate arbitrary paths. A target with several
authored assertions has one edge record, with all occurrences inspectable
through `relation get` as specified in the relationship contract.

Order roots by path/start byte, rules by name (one request check has no
rule-name ordering), and preserve the evaluation ordering within each result.
Unavailable roots/steps remain explicit; do not silently omit them when
bounds are reached. After work exhaustion, one terminal issue record states
the first unevaluated root/rule and that the remaining selected suffix is
unavailable; do not manufacture millions of placeholder rows.

Markdown groups this stream as source-linked root/rule results and check
tables. Show count gaps, rejected targets and the first missing downstream
step using endpoint-facing relation labels. A draft verification is visibly
non-qualifying even when an approved verification makes the root pass.
A second-hop gap identifies its verification and evidence obligation.
External edges display their address and terminal kind, never an invented
item status or source location.

Do not calculate a global percentage. Matrix results add `summaries`, one
entry per evaluation identity, with `selected`, `not_applicable`, `passed`,
`failed`, `unavailable` and `counts_exact`. These are whole-view counts before
pagination; report lower bounds when evaluation is incomplete.
For a complete rule, applicable denominator is passed + failed.
Request checks have a separate summary and impose no project validity.
The grammar's fixture table is also the expected matrix-state table.

## Specification records and source navigation

A path-only or all selection includes matching documents and their narrative.
An item filter (IDs, flavours or fields) selects item content only; paths then
restrict those items. Include ancestor heading labels as breadcrumbs, not
unselected ancestor bodies. State this narrative omission in the selection
description. Overlapping paths/filters never duplicate an authored source span.
Order selected documents by path and content by start byte.

Each `content` record contains the existing discovery node descriptor,
ordered item metadata when applicable, and consecutive original Markdown
content. Use get's UTF-8 content ranges and ordered metadata fragment ranges
for oversized nodes, with the same no-skip/no-overlap reconstruction contract.
Do not return a document span and duplicate all of its nested item bodies.
Item boundaries inside selected documents retain source identities through
`item` marker records at their source positions.
A `relationship` record attaches each selected item's incident semantic edge,
canonical direction/endpoint-facing label, endpoint descriptor, occurrence
count and source-inspection request. Emit a shared edge once per selected
endpoint to preserve each endpoint's context; do not duplicate it per assertion.
Mark neighbours outside selection and do not recursively include their bodies.
No coverage or policy pass is implied by these relationship records.

Markdown preserves selected authored prose and renders item identity, metadata,
source location, and relationship context separately. Preserve code/literal
examples. Supported internal references link to the original canonical
destination; no generated destination may retarget a reference.
Links to canonical files use project-relative paths; save the rendered file
at the project root for those relative links to resolve. Print that base
assumption in the document header. Rebase relative Markdown destinations from
each source document to that root in rendered prose, retaining fragments;
JSON retains the original source text. External URLs remain unchanged.
If a reference cannot be resolved, retain its text and report the issue instead
of fabricating a link. Include source line numbers as visible labels; use
existing source heading/explicit anchors when present, not invented line anchors.

Every rendered page identifies scope, output continuation and evaluation
completeness. Content fragments carry a visible continued marker; Markdown
pages are readable portions, while JSON ranges support exact reconstruction.
Regeneration from unchanged inputs is byte-deterministic: no generation time,
random identifiers or current working-directory-dependent content. Changing
one selected item updates its next generated content; generation never writes
back or synthesizes missing requirements/relations.
:::

:::mara design DES-TRACE-CONTRACT-COMPATIBILITY
:mid: 01M2JP4QJG3D8MNQE6MPMAG17E
:title: Version rule and view contracts without reinterpreting current state
:satisfies: REQ-SCHEMA-EVOLUTION
:satisfies: REQ-CURRENT-STATE-RULES

The rule/diagnostic/view contracts extend the schema-3 relationship baseline
in [[DES-RELATION-COMPATIBILITY]]. They describe planned 0.3 behavior;
do not advance the active schema or executable during contract authoring.

| Surface | Compatibility boundary |
|---|---|
| Schema | Keep the planned format 3; add optional rules, relation cardinality and acyclic policies. Absent policies impose no obligations. |
| Documents | No new marker or metadata syntax. Status and other rule inputs are ordinary project-defined fields. |
| Project configuration | Remains format 1; no saved views or work-limit settings are introduced. |
| Validation JSON | Start format_version 1 for project/item/schema validation and operation errors, replacing unversioned results. Explicit completeness, codes, severities, counts and continuation require client updates. |
| Trace JSON | Start a separate format_version 1 family for matrix/specification results and errors. |
| Discovery/relationship JSON | Retain the independently planned versions in the relationship compatibility contract. |
| MCP | Reflect matching domain inputs/results in tool schemas; leave transport negotiation independent. |

Validation clients must read severity, evaluation_complete, valid and
continuation, rather than equating a nonempty diagnostic list with failure or
one page with complete reporting. Location path normalization and schema
validation's common envelope are explicit migration changes. Discard old
cursors on upgrade; MIDs and existing item/source references retain their
documented identity rules.

Migrate custom format-2 schemas using the recoverable workflow in the
relationship compatibility contract. With no rules or graph policies, the
rule extension adds no migration beyond that baseline. To adopt policy,
deliberately declare any needed custom fields, add the intended rules and
constraints, and compare validation output before and after. Do not populate
statuses, owners, evidence, or links automatically.

Example: a customized schema with optional requirement status migrates to 3
without making drafts invalid. Adding the approved-requirement rule then
reports missing owners/approved verifications only for approved requirements.
Changing its severity to warning preserves predicate results while allowing
a complete project with only those failures to remain valid.
An unknown status literal or rule field is a configuration error; restore the
checkpoint or correct the declaration, never report a completed migration.
Preserve all IDs/MIDs and unrelated source bytes. New migration automation and
broader vocabulary transformations remain separate implementation/design work.

## Future transition extension exercise

Current-state rules always inspect the supplied current snapshot, even when
run after a direct Markdown edit. They do not imply that an item moved through
allowed states, had prior approval, or has fresh evidence.

A future, explicitly versioned sibling namespace can add previous/current
context without changing a rules entry. The following is a compatibility
sketch only, rejected as an unknown key by schema format 3:

```yaml
transition_rules:
  approve_requirement:
    select: {flavours: [requirement]}
    when:
      all:
        - previous: {field: status, equals: draft}
        - current: {field: status, equals: approved}
    require:
      current: {field: owner, nonblank: true}
```

A future comparator would pair the same MID in two explicit revisions;
renaming its human ID would not create a different identity. For a draft to
approved pair the sketch requires current owner; unchanged approved to
approved does not activate this transition rule, while the existing
approved_requirement current-state rule still activates.
Missing previous/current items, added/deleted items, baseline provenance and
evidence freshness need a later contract. Do not interpret a missing previous
snapshot as a successful transition or add executable transition support in
0.3. This exercise follows [[ADR-CURRENT-STATE-BEFORE-TRANSITIONS]].
:::

:::mara decision ADR-DECLARATIVE-TRACE-BASELINE
:mid: 01M2JP4QMA7C4Z6H382XC5AFFG
:title: Keep trace policy explicit and distinguish incomplete evaluation
:justifies: DES-TRACE-RULE-GRAMMAR
:justifies: DES-TRACE-GRAPH-CONSTRAINTS
:justifies: DES-TRACE-DIAGNOSTIC-INTERFACE
:justifies: DES-TRACE-VIEW-INTERFACES
:justifies: DES-TRACE-CONTRACT-COMPATIBILITY

Adopt the schema-3 grammar, explicit graph policies, diagnostic interface and
request-selected views in this document.

Missing values fail comparisons; explicit absence checks express the opposite
intention without making missing lifecycle data accidentally satisfy a value
test. Every over an empty related set passes; minimum counts express existence
separately. Combining them supports both optional-but-qualified and mandatory
coverage without overloading one operator.

Rules default to error and may be warnings. Invalid prerequisites and work
limits remain errors because treating an unperformed check as a policy warning
could claim a full pass. Keep complete evaluation distinct from output
pagination so small responses cannot hide incomplete analysis.

Use one project-owned schema and the existing validation entry points.
Explicit matrix/specification requests with JSON and Markdown meet the current
workflow without saved-view state or a general query/workflow engine.
Use bounded steps, logical work and output pages rather than unbounded graph
expansion. Permit a fresh higher-budget request within a fixed ceiling;
continuation reads output rather than silently resuming partial validation.

Version public result families independently. Add future transition context
through a separate versioned namespace; silently reinterpreting current-state
rules would change projects' existing policy after an upgrade.
:::
