# Rules, diagnostics and trace views

Contracts for the planned 0.3 implementation, extending
[traceability](traceability.mara.md) and the accepted
[relationship contracts](relations.mara.md). Examples describe intended
results, not checks executed by the 0.2 binary. The active schema and executable
remain unchanged. Project examples require the illustrated vocabulary; they
do not add lifecycle policy to bundled templates or existing projects.

The accepted language foundation is native SHACL with CEL expressions, supported
by [[EVD-SHACL-CEL-SPIKE]]. The contracts below replace the earlier unshipped
custom YAML predicates; loading the binding and enforcing all bounds remain
implementation work.

:::mara design DES-TRACE-RULE-GRAMMAR
:mid: 01M2JNZMJ0VT20GWH4HF6DBBC5
:title: Bind native SHACL shapes and CEL predicates to project items
:satisfies: REQ-CURRENT-STATE-RULES
:satisfies: REQ-TRACE-COVERAGE
:satisfies: REQ-BOUNDED-TRACE-CHAINS

Adopt CEL expressions for local predicates and SHACL shapes for relationship
obligations. This replaces the unshipped Mara-specific YAML predicate grammar;
the former `field`, `all`, `any`, `related`, `qualifies` and `every`
mappings are not a second supported language. [[EVD-SHACL-CEL-SPIKE]] verifies
a native Rust integration using cel 0.14.5 and shacl 0.3.21.

## Native source files and identity

Store rules in UTF-8 Turtle (`.ttl`) with CEL expressions as string literals.
Project configuration references explicit project-relative files; no rule bodies
are embedded in schema YAML. The planned configuration addition is:

```toml
[rules]
format_version = 1
files = ["rules/traceability.ttl"]
```

[[DES-TRACE-CONTRACT-COMPATIBILITY]] defines the enclosing project format.
Absent `rules` or an empty file list means no conditional rules. Resolve paths
from the project root, require existing regular `.ttl` files within that root,
reject duplicates and do not expand globs. Combine the listed files into one
shapes graph, preserving every source occurrence. File order is not override
precedence. Conflicting single-valued parameters are `rule_invalid`; repeated
identical RDF triples have one meaning. Loading an enabled file must not silently
fail or use cached rules from another snapshot.

Only Turtle is supported initially. Do not fetch imports, remote contexts,
schemas or namespace IRIs. A namespace is an identifier, not a download request.
References between named shapes resolve within the combined graph; blank nodes
are file-local. Require absolute shape IRIs or an explicit absolute `@base`
so a checkout's filesystem location cannot change rule identity.

An enabled rule is a named `sh:NodeShape` with `sh:targetClass` naming one or
more declared flavour classes. Multiple targets select their union. Shapes
without targets are reusable obligations, not independently executed rules.
The expanded root shape IRI is the rule's identity; prefix labels and filenames
are not identity. Persisted `sh:targetNode`, implicit class targets and other
target mechanisms are outside this profile. Exact item selection belongs in
requests. Unknown flavours, relation vocabulary and unsupported constraints
must be rejected rather than ignored.

## Mara binding to the standard languages

Use `m: <urn:mara:rules:1:>`, `f: <urn:mara:flavour:>` and
`r: <urn:mara:relation:>` for the adapter, flavour and canonical relation
namespaces. These namespaces are part of the version-1 binding.

Project each internal item as `urn:mara:mid:MID`, with `rdf:type` for its
declared flavour. Project each canonical relation once, using its canonical
name as the predicate local part. Normalize inverse authoring first; SHACL
inverse paths choose incoming traversal. Symmetric edges expose both directions
but a self-edge appears once. Retain source occurrences outside the RDF set.
Flavour and relation names are encoded as UTF-8 percent-escaped IRI suffixes
when needed; do not normalize case or substitute alias names.

External endpoints remain distinct terminal nodes keyed by the exact normalized
address from [[DES-CANONICAL-TRACE-RELATIONS]], using
`urn:mara:external:` plus its percent-encoded address. They have no item flavour
or CEL field context. Count them where the relation permits; do not traverse
through them or interpret their URI as fetched RDF.

The host binding adds only these source properties:

| Property | Allowed location and meaning |
|---|---|
| `m:when` | At most one CEL string on an enabled rule root. False means not applicable; omitted means true. |
| `m:cel` | At most one CEL string on an internal-item node shape. It is an obligation, conjunctive with that shape's SHACL constraints. |
| `m:paths` | Zero or more path/subtree strings on an enabled rule root, using existing retrieval path rules. OR within paths, AND with flavour targets. Omitted means all project paths. |

These properties are Mara extensions, not standard SHACL/CEL vocabulary.
A generic SHACL validator may ignore them; it cannot validate an authored Mara
rule file correctly without the adapter. Reject unknown executable vocabulary,
SPARQL/JavaScript constraints, external code, recursive shape references and
unbounded property paths in this initial profile.

CEL receives `node`, a map of schema-declared custom fields only. Scalars
retain schema meaning: strings/enums are CEL strings, booleans are bools,
integers are signed CEL ints and floating values are doubles. Repeatable fields
are lists in authored order; absent optional fields have no map key. An authored
empty string remains present. IDs, titles and relations are not implicit
custom fields. Unsupported numeric values are invalid inputs, never coerced.

Use standard CEL syntax, operators, collection macros and Boolean/error
semantics, without user-defined functions, I/O, clock or network bindings.
Expressions must produce bool. Validate syntax and available declarations
before running item checks; dynamic map lookups still follow CEL runtime
semantics. Invalid authored metadata prevents evaluating the affected item;
it is not represented as absence. Schema enums constrain authored values:
a comparison to another string is legal CEL and can simply evaluate false.

For optional fields, write explicit guards:

```cel
has(node.status) && node.status == "approved"
```

An unguarded absent map key is an evaluation error, not implicit false.
Within one expression, CEL can determine a Boolean result despite another
operand's error, for example a true alternative. Do not rewrite CEL's
operators to the former YAML composition semantics. Across independently
required expression/shape checks, retain evaluation failures separately so
another qualifying target cannot hide them.

## Relationship shapes and evaluation

The initial SHACL profile supports flavour classes, predicate and inverse
relation paths, `sh:property`, `sh:node`, `sh:minCount`, `sh:maxCount`,
`sh:qualifiedValueShape`, qualified minimum/maximum, `sh:in`, and
`sh:and`/`sh:or`/`sh:not`. `sh:name`, `sh:description`,
`sh:message` and `sh:severity` provide annotations. Compound traversal is
expressed with nested shapes, not arbitrary path expansion. The binding's
SHACL subset is explicit; it is not a claim to implement all SHACL features.

A property shape's selected set is the distinct endpoints of its path.
Flavour/status restrictions belong in the qualifying node shape. Thus
`selected_count` includes nonqualifying endpoints; `qualifying_count`
includes only those satisfying the complete qualifier. Use `sh:node` when
every selected endpoint must conform. An empty set satisfies `sh:node`;
an explicit minimum requires existence. Counts apply at the immediate hop,
not to all downstream paths. Two verifications sharing evidence remain two
first-hop endpoints.

Only schema-compatible relations and internal CEL contexts are valid. An
external-capable path may use plain counts; CEL qualification must be scoped
to a declared internal flavour before the adapter evaluates it. Flavour
mismatch makes the node nonqualifying without binding nonexistent item fields.
Unconstrained graph cycles do not change finite nested-shape semantics.
[[DES-TRACE-DIAGNOSTIC-INTERFACE]] defines depth and work limits.

Root severity is `sh:Violation` by default, mapped to error; explicit
`sh:Warning` maps to warning. The root owns the policy severity of its
reusable obligations. Nested severity overrides and other severities are
rejected initially. SHACL conformance and Mara validity are distinct:
a complete warning-only failure remains valid in Mara.

The adapter may materialize each CEL-qualified MID set as `sh:in` before
native SHACL execution, as tested in [[EVD-SHACL-CEL-SPIKE]]. The set is
request-local generated data, never persisted policy. Preserve a separate
error ledger and authored-shape/source mapping through lowering. Evaluate
only contexts selected by root scope and explicit relationship obligations;
unrelated items must not generate CEL errors for an inapplicable rule.

## Worked native rules

This complete Turtle example assumes optional project-defined statuses, an
optional string owner, and the directed relations shown by the paths.

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix m: <urn:mara:rules:1:> .
@prefix f: <urn:mara:flavour:> .
@prefix r: <urn:mara:relation:> .
@prefix rule: <urn:example:rules:> .

rule:approved_requirement a sh:NodeShape ;
    sh:targetClass f:requirement ;
    m:when """has(node.status) && node.status == "approved" """ ;
    m:cel """has(node.owner) && node.owner.matches(r'(?s).*\\S.*')""" ;
    sh:property rule:verification_count .

rule:verification_count a sh:PropertyShape ;
    sh:path [ sh:inversePath r:verifies ] ;
    sh:qualifiedValueShape rule:approved_verification ;
    sh:qualifiedMinCount 1 .

rule:approved_verification a sh:NodeShape ;
    sh:class f:verification ;
    m:cel """has(node.status) && node.status == "approved" """ .

rule:accepted_design a sh:NodeShape ;
    sh:targetClass f:design ;
    m:when """has(node.status) && node.status == "accepted" """ ;
    sh:property [
        sh:path r:satisfies ;
        sh:qualifiedValueShape [ sh:class f:requirement ] ;
        sh:qualifiedMinCount 1
    ] .

rule:mitigated_risk a sh:NodeShape ;
    sh:targetClass f:risk ;
    m:when """has(node.status) && node.status == "mitigated" """ ;
    sh:property [
        sh:path [ sh:inversePath r:mitigates ] ;
        sh:qualifiedValueShape [ sh:class f:design ] ;
        sh:qualifiedMinCount 1
    ] .
```

| Fixture | Required outcome |
|---|---|
| Approved requirement, owner present, only a draft verification | Failed: qualifying count 0, selected count 1. |
| Add an approved verification and repeat its authored edge | Passed: qualifying count 1, selected count 2. |
| Add `sh:node rule:approved_verification` to verification_count | Failed because of the draft; qualified minimum still passes. |
| No verification and only `sh:node`, with no minimum | Passed. |
| No verification and minimum one | Failed minimum. |
| Approved requirement with blank/missing owner | Failed local obligation. |
| Draft requirement or absent optional status | Not applicable under the guarded condition. |
| Accepted design without a satisfies edge | Failed; one requirement edge satisfies the minimum. |
| Mitigated risk without incoming mitigation | Failed; one design mitigation satisfies the minimum. |

To require passing evidence for each qualifying verification, add these triples
to the same file:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix m: <urn:mara:rules:1:> .
@prefix f: <urn:mara:flavour:> .
@prefix r: <urn:mara:relation:> .
@prefix rule: <urn:example:rules:> .

rule:approved_verification sh:property [
    sh:path [ sh:inversePath r:evidences ] ;
    sh:qualifiedValueShape [
        sh:class f:evidence ;
        m:cel """has(node.outcome) && node.outcome == "passed" """
    ] ;
    sh:qualifiedMinCount 1
] .
```

An approved verification without passing evidence does not qualify at the first
hop. Adding passing evidence closes that gap. A draft verification remains
nonqualifying even with evidence. These checks inspect recorded assertions;
they do not execute verification or establish evidence trustworthiness/freshness.

## Evaluation states

Retain `not_applicable`, `passed`, `failed` and `unavailable` at the
item/rule boundary. Invalid source, unresolved identities, invalid definitions
and exhausted bounds must not become zero counts or false applicability.
Continue independent checks whose prerequisites are available. Any unavailable
required child makes the containing shape/rule unavailable, including a
SHACL alternative with another passing child. This is Mara completeness
reporting; ordinary Boolean evaluation inside a single CEL expression remains
standard CEL.

Ordinary source edits and structured mutations are evaluated identically;
policy failure does not introduce a new mutation gate. Rust library integration
is tested, but loading this persisted binding, precise source mapping and
enforcing whole-engine work accounting remain implementation obligations.

References: [SHACL](https://www.w3.org/TR/shacl/),
[Turtle](https://www.w3.org/TR/turtle/),
[CEL language](https://github.com/cel-expr/cel-spec/blob/master/doc/langdef.md)
and [CEL Policy](https://github.com/cel-expr/cel-policy).
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
and/or `maximum`, with optional `severity` defaulting to error. Bounds are nonnegative integers, minimum cannot exceed maximum, and at least
one bound is required. Directed declarations
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

These remain structural relation declarations, not conditional-expression
syntax. They introduce no alternative predicate language. Cardinality may
lower to SHACL counts, while acyclicity retains the finite graph pass and
witness contract above; adopting SHACL does not enable unbounded rule paths.
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
Charge one logical unit for each item/selection test, SHACL constraint
invocation at a node, SHACL value/list membership comparison, semantic-edge
examination, graph-policy vertex/edge
visit and evaluated CEL AST node (including each comprehension iteration).
String/collection builtins additionally charge their input byte/element counts;
charge before the operation. Schema validation charges each visited declaration,
SHACL constraint and CEL AST node. Specification generation charges each source
node/edge before fragmentation. Charge logical visits even when cached.
Counts alone at the CEL/SHACL invocation boundary are insufficient: enforce
the budget inside both evaluators or report the operation unavailable. Do not
claim budget compliance from the successful library experiment alone.

Sort items by path/start byte, rules by expanded shape IRI, and relation edges
by canonical kind and endpoint identity. Order independent SHACL obligations
by source path/start byte; for repeated identical triples use the earliest
source location. RDF list members preserve list order. Blank-node labels from
a parser are not stable ordering keys. CEL uses its standard evaluation rules;
pin the evaluator/cost-model revision in the snapshot and charge the executed
AST, not speculative branches. Parallel scheduling must not change the
observable evaluated prefix. Loading/parsing source is outside this logical
evaluation budget and retains existing read/error behavior.

Allow at most eight nested relationship steps, SHACL shape-reference depth 32
and CEL AST depth 32 (root depth 1). Reject deeper definitions, recursive shape
references and unbounded paths as invalid configuration. Graph cycle policies
visit the finite normalized graph.

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
schema/corpus/project/rule-source/options changes as `stale_cursor`. Re-evaluation may
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
`start_byte`/`end_byte` and JSON Pointer `pointer` for TOML/YAML configuration when applicable.
Turtle locations use actual source spans, not invented JSON pointers.
CEL expression spans map through Turtle string escapes back to authored bytes.
Only available coordinates are populated; never invent line numbers.
Retain legacy `path` and `line` fields as aliases of location coordinates.
An external configured schema path remains absolute.

Item diagnostics add `item:{id,mid}` when unambiguous; configuration/rule
diagnostics add `rule`, the expanded root shape IRI, when known.
Rule failures add `obligation:{shape,component,source}`: shape is its expanded
IRI or a snapshot-bound opaque reference for a blank node; component is the
SHACL component IRI or `urn:mara:rules:1:cel`; source is the native definition's
location. `details.kind` is `cel|class|minimum|maximum|every|and|or|not|in`.
CEL details identify the authored expression and result/error, not a fabricated
list of custom field operators. Count details include
`selected_count`, `qualifying_count` and the violated bound.
Relationship explanations retain canonical relation, direction and
endpoint-facing label, plus item/edge references. Locations of all assertions
remain inspectable via relation get, not an unbounded inline list.

Emit one `rule_failed` per failed item/rule pair, pointing to the first
unsatisfied leaf in the obligation order above that contributes to the root failure.
Do not emit failures for unsuccessful alternatives of a passing SHACL or.
The matrix exposes the remaining check results, including all failed
alternatives when SHACL or fails. Unavailable rules produce
`evaluation_unavailable` referencing their prerequisite diagnostics or the surfaced CEL error instead
of a fabricated policy failure. Standard CEL Boolean results are not themselves
unavailable merely because an unneeded operand could fail. An exhausted request emits one global
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
Schema validation also loads the configured Turtle sources, checks the supported
SHACL/binding profile, CEL syntax/declarations and graph policies, not runtime
item predicates. Dynamic CEL accesses are checked when evaluated.

Sort diagnostics by scope (project, schema, document, item), path, start byte
(or line when byte is absent; missing coordinates first), item MID, rule,
obligation source/shape/component and code, with message as final deterministic tie-breaker.
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
| `trace matrix --flavour requirement --rule urn:example:rules:approved_requirement` | `trace_matrix {flavours:["requirement"], rules:["urn:example:rules:approved_requirement"]}` |
| `trace matrix --id REQ-A --check-file rules/coverage.ttl --shape urn:example:rules:coverage` | `trace_matrix {ids:["REQ-A"], check:{files:["rules/coverage.ttl"], shape:"urn:example:rules:coverage"}}` |
| `trace specification --path docs/` | `trace_specification {paths:["docs/"]}` |
| `trace specification --flavour requirement --field status=approved` | `trace_specification {flavours:["requirement"], fields:[{key:"status",value:"approved"}]}` |

Both accept `--all`, repeatable `--id`, `--flavour`, `--field`,
`--path`, and `--limit`, `--cursor`, `--max-work`.
Matrix additionally requires either repeatable `--rule` / nonempty `rules`,
or a request-local check, never both. Rule values are exact expanded root shape
IRIs from enabled sources; unknown IRIs are errors. Prefix abbreviations are
source syntax, not request aliases.

For a check, CLI accepts repeatable `--check-file` and one `--shape`;
MCP accepts `check:{files:[...],shape:IRI}`. Load those native Turtle sources
using the rule-file contract and require the designated named node shape.
Apply it unconditionally to the request's selected roots; reject root targets,
`m:when` and `m:paths` on the designated check, and do not execute other
targeted shapes from the supplied files. Its referenced obligations and CEL
expressions follow the same typing, profile and work limits as persisted rules.
The files define reusable constraints, not a saved view: selection stays in
the request. They impose no project-validation policy unless separately enabled
in project configuration.

Named rules retain their own selection and applicability, intersected with
view roots. Output distinguishes persisted-rule and request-check identity;
a request check cannot override a persisted rule. The former JSON `related`
check object is withdrawn with the custom predicate grammar.

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
| `check` | `reference`, `root`, `evaluation`, `obligation` shape/component/source descriptor, `context`, `parent` check reference (null at root), `state`, and `condition` predicate; relationship checks add `counts` and `every`. |
| `edge` | `check` reference, canonical `edge`, endpoint-facing `label`, `direction`, `endpoint`, `qualification`, `every`, and `occurrence_count`. |
| `issue` | `diagnostic` preventing complete evaluation. |

`evaluation` is `{kind:"rule",shape:"urn:example:rules:approved_requirement"}`
or `{kind:"check",shape:"urn:example:rules:coverage"}`. `root` and item endpoints use discovery item descriptors;
external endpoints use the relationship contract's external descriptor.
Every record carries `kind`. `condition` identifies the SHACL component and its parameters, or the CEL
source expression. Nested shape obligations have their own check records;
do not expand CEL's internal AST into a new public predicate language.
A check's `counts` contains `selected`,
`qualifying`, `minimum` and `maximum`; omitted bounds and unavailable totals
are null. `every` and `qualification` use the predicate states or null when
that test is not requested. `context` is an ordered array of canonical edges.

An endpoint context is the root plus the ordered canonical edges to that
check; at most eight hops. `check` references are snapshot-bound opaque
identifiers, not durable item identities. They connect records across pages.
Counts belong to the immediate check: include selected and qualifying totals,
declared minimum/maximum, and every state where present. Counts are null
when unavailable, not misleading zeros. CEL checks retain expression source and result/error; source navigation supplies
authored field values without pretending arbitrary CEL has a field/operator pair.

Emit one result for each root/rule pair, including not-applicable roots.
Only applicable rules have check records. Emit check records in deterministic shape-obligation
preorder under [[DES-TRACE-DIAGNOSTIC-INTERFACE]]; relationship edge records follow their owning check in canonical
endpoint order, each followed by its nested checks. Emit failed alternatives
for explanation even when the parent SHACL or passes; distinguish child state
from root state. Never enumerate arbitrary paths. A target with several
authored assertions has one edge record, with all occurrences inspectable
through `relation get` as specified in the relationship contract.

Order roots by path/start byte, rules by expanded IRI (one request check has no
persisted-rule ordering), and preserve the evaluation ordering within each result.
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
The rule design's fixture table is also the expected matrix-state table.

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
| Schema | Keep the planned format 3 for vocabulary/relationships and structural cardinality/acyclic declarations. Conditional rules are native Turtle files, not a top-level YAML rules mapping. Absent policies impose no obligations. |
| Documents | No new marker or metadata syntax. Status and other rule inputs are ordinary project-defined fields. |
| Project configuration | Continue accepting format 1 for projects without rule sources. Enabling native rules requires format 2 and the optional rules table in DES-TRACE-RULE-GRAMMAR; reject unknown/unsupported versions. No saved views or persisted work limits. |
| Rule binding | Start format_version 1 inside the rules table. It selects the supported Turtle/SHACL profile, CEL binding and urn:mara:rules:1: vocabulary. This is independent of W3C or crate release numbers. |
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
relationship compatibility contract. Without native rules, no project-config
migration is required beyond the relationship baseline. To enable rules, declare any needed custom fields,
create and review Turtle sources with embedded CEL, change project
`format_version` to 2, and add `[rules]` with binding version 1 and explicit
file paths. Preserve existing project/content configuration. Validate source
loading and compare policy output before and after. Do not populate
statuses, owners, evidence, or links automatically.

Example: a customized schema with optional requirement status migrates to 3
without making drafts invalid. Adding the approved-requirement rule then
reports missing owners/approved verifications only for approved requirements.
Changing its severity to warning preserves predicate results while allowing
a complete project with only those failures to remain valid.
Malformed Turtle, unsupported bindings and invalid CEL definitions prevent
successful adoption. A valid string comparison to a non-enum value can be false
under standard CEL; invalid authored enum values remain field errors.
Missing-field expressions need explicit `has` guards; do not translate absence
to false outside the expression. Restore the checkpoint or correct failed
declarations, never report a completed migration.
Preserve all IDs/MIDs and unrelated source bytes. New migration automation and
broader vocabulary transformations remain separate implementation/design work.

## Future transition extension exercise

Current-state rules always inspect the supplied current snapshot, even when
run after a direct Markdown edit. They do not imply that an item moved through
allowed states, had prior approval, or has fresh evidence.

Future transition policy needs a separately versioned binding that explicitly
introduces previous/current context. The following CEL is an illustrative
future condition, not an executable current-state definition:

```cel
has(previous.status) && previous.status == "draft" &&
has(current.status) && current.status == "approved"
```

Its future obligation could inspect `has(current.owner)` and a nonblank
owner expression. The current binding declares only `node`; `previous`
and `current` are rejected as undeclared variables. Do not add a competing
YAML transition grammar or silently rebind current-state expressions.

A future comparator would pair the same MID in two explicit revisions;
renaming its human ID would not create a different identity. For a draft to
approved pair the future obligation requires current owner; unchanged
approved to approved does not activate this transition rule, while the existing
approved_requirement current-state rule still activates.
Missing previous/current items, added/deleted items, baseline provenance and
evidence freshness need a later contract. Do not interpret a missing previous
snapshot as a successful transition or add executable transition support in
0.3. This exercise follows [[ADR-CURRENT-STATE-BEFORE-TRANSITIONS]].

The earlier custom YAML rule/check syntax was never shipped. Remove it from
the accepted design rather than supporting or automatically migrating it.
Existing item documents, MIDs and active format-2 schema files remain unchanged
during this documentation-only adoption. A future expansion of the binding's
SHACL features or CEL environment needs an explicit compatibility decision.
:::

:::mara decision ADR-DECLARATIVE-TRACE-BASELINE
:mid: 01M2JP4QMA7C4Z6H382XC5AFFG
:title: Keep trace policy explicit and distinguish incomplete evaluation
:justifies: DES-TRACE-RULE-GRAMMAR
:justifies: DES-TRACE-GRAPH-CONSTRAINTS
:justifies: DES-TRACE-DIAGNOSTIC-INTERFACE
:justifies: DES-TRACE-VIEW-INTERFACES
:justifies: DES-TRACE-CONTRACT-COMPATIBILITY

Adopt CEL local expressions and SHACL relationship shapes with embedded CEL
strings in native Turtle files, explicitly referenced from project configuration.
Replace the unshipped custom YAML predicate grammar. Keep the graph policies,
diagnostic interface and request-selected views in this document.

Native source formats avoid maintaining another expression syntax or translating
SHACL into YAML. Keep expressions beside their constraints instead of adding
separate CEL-file references. [[EVD-SHACL-CEL-SPIKE]] provides the native Rust
feasibility evidence. The versioned Mara adapter owns item projection,
applicability, source mapping and completion reporting; generic SHACL engines
cannot be assumed to honor its CEL properties.

Use standard CEL semantics. Guard optional fields explicitly with `has`;
unguarded missing map entries are errors, and ordinary CEL Boolean operators
retain their standard error behavior. Do not add a compatibility interpreter
to preserve the old implicit-false comparisons. Every over an empty related set
passes; minimum counts express existence separately. Combining them supports both optional-but-qualified and mandatory
coverage without overloading one operator.

Rules default to error and may be warnings. Invalid prerequisites and work
limits remain errors because treating an unperformed check as a policy warning
could claim a full pass. Keep complete evaluation distinct from output
pagination so small responses cannot hide incomplete analysis.

Keep one project-owned vocabulary schema, explicitly enabled native rule
sources and the existing validation entry points.
Explicit matrix/specification requests with JSON and Markdown meet the current
workflow without saved-view state or a general query/workflow engine.
Use bounded steps, logical work and output pages rather than unbounded graph
expansion. Permit a fresh higher-budget request within a fixed ceiling;
continuation reads output rather than silently resuming partial validation.

Version public result families independently. Add future transition context
through a separate versioned namespace; silently reinterpreting current-state
rules would change projects' existing policy after an upgrade.
:::
