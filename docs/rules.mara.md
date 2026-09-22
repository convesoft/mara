# Rules, diagnostics and trace views

Contracts for the planned 0.3 implementation, extending
[traceability](traceability.mara.md) and the accepted
[relationship contracts](relations.mara.md). Examples describe intended
results, not checks executed by the 0.2 binary. The active schema and executable
remain unchanged. Project examples require the illustrated vocabulary; they
do not add lifecycle policy to bundled templates or existing projects.

The accepted language foundation is SHACL Core, authored in YAML with generated
bindings and verified by [[EVD-YAML-SHACL-SPIKE]]. The contracts replace the
earlier unshipped formats; the complete loader and bound enforcement remain
implementation work.

:::mara design DES-TRACE-RULE-GRAMMAR
:mid: 01M2JNZMJ0VT20GWH4HF6DBBC5
:title: Bind YAML-authored SHACL Core shapes to project items
:satisfies: REQ-CURRENT-STATE-RULES
:satisfies: REQ-TRACE-COVERAGE
:satisfies: REQ-BOUNDED-TRACE-CHAINS

Use SHACL Core for local field conditions and relationship obligations,
authored only in YAML with Mara-generated bindings. This replaces the unshipped
Turtle and custom-predicate formats. [[EVD-YAML-SHACL-SPIKE]] verifies native
Rust YAML → JSON-LD → RDF → SHACL evaluation without a Turtle intermediate.

## Source files and identity

Store rules in UTF-8 `.yaml` or `.yml` files. Each contains one YAML document:
a shape mapping or a sequence of shape mappings. Use string keys,
JSON-compatible scalar values, mappings and sequences. Duplicate mapping keys,
custom tags, merge keys and cyclic aliases are invalid. Ordinary aliases may
reuse a value; expanded constraints retain their authored occurrence/source
mapping. Null is not a missing constraint parameter.

Project configuration references explicit project-relative files; rule bodies
are separate from vocabulary schema YAML. The planned addition is:

```toml
[rules]
format_version = 1
files = ["rules/traceability.yaml"]
```

[[DES-TRACE-CONTRACT-COMPATIBILITY]] defines the enclosing project format.
Absent `rules` or an empty file list means no conditional rules. Resolve paths
from the project root, require existing regular YAML files inside that root,
reject duplicate paths and do not expand globs. Combine definitions into one
shapes graph, preserving every authored occurrence. File order is not override
precedence. Repeated named-shape descriptions add constraints; conflicting
single-valued parameters are `rule_invalid`, while repeated identical RDF
triples have one meaning. Loading failure must not reuse cached rules from
another snapshot.

YAML is the sole rule input. There is no Turtle input, intermediate-file or
export contract, and no public JSON-LD input. Contexts are generated in memory:
reject authored `@context`, other raw JSON-LD keywords, context-file references
and vocabulary overrides. Do not fetch imports, schemas or namespace IRIs.

An enabled rule is a named `NodeShape` with `targetClass` naming one or more
declared flavours. Multiple targets select their union. Targetless shapes are
reusable obligations/conditions, not independently executed rules. Require
`id` on enabled roots and named reusable definitions. Use `rule:name`,
expanded to `urn:mara:rule:name`, or an absolute IRI; reject relative IDs.
The expanded IRI is identity; it must not depend on filename, checkout path or
definition order. Unnamed nested shapes have file-local blank-node identity.

Authored `targetNode`, implicit class targets and other target mechanisms are
outside this profile. Exact item selection belongs in requests. Only the
adapter may introduce focus-node targets for an evaluation.

## Generated bindings and definition validation

The binding has a fixed versioned SHACL/datatype vocabulary and derives project
names from the current schema. No user-maintained context or second vocabulary
registry is required. Convert the parsed YAML to JSON-LD using that context,
then construct the RDF shapes graph. This is a Mara YAML authoring profile with
standard SHACL semantics, not a standalone context-free YAML-LD document.

| Authored position | Binding |
|---|---|
| `id`, `type` | JSON-LD identity/type; types are `NodeShape` or `PropertyShape`. |
| Supported constraint keys | Corresponding SHACL properties, retaining their standard parameter meaning. |
| `targetClass`, `class` | Schema-declared flavour names; a scalar or sequence denotes one or several classes. |
| `path` | A declared field/canonical relation name, or `{inversePath: relation}`. |
| `datatype` | `string`, `integer`, `double`, `boolean` map to XML Schema datatypes. |
| `node`, `not`, `qualifiedValueShape` | One nested shape or named shape reference. |
| `property` | A sequence of property shapes/references; each contributes an obligation. |
| `and`, `or` | Sequences of shapes/references, encoded as RDF lists. |
| `in` | An RDF list of literal values; not several independent in constraints. |
| `hasValue` | One literal value, using standard JSON-LD scalar conversion. Quote strings that YAML would otherwise parse as another type. |
| `severity` | `Violation` or `Warning`; omitted root severity is Violation. |
| `name`, `description`, `message` | String annotations. |

Field names are bound in path-value context, flavour names in class-value
context, and datatypes in datatype-value context. They cannot replace rule
keywords or reinterpret literal strings. For example, `path: class` can select
a custom field while `class: requirement` still constrains the flavour;
`hasValue: requirement` remains a literal string.

Use unqualified path names only when unambiguous. `field:name` explicitly
selects a field; `schema:name` selects a canonical schema relation. Preserve the
existing `schema:`/`builtin:` distinction, without adding built-in discovery
edges to the rule graph: `builtin:` paths are unsupported in this profile.
Reject ambiguity rather than choose by lookup order. Existing declaration
collision restrictions in [[DES-RELATION-AUTHORING]] still apply. Incoming
traversal uses `inversePath`; relation aliases do not create new RDF predicates.

Generate bindings deterministically from the schema and binding version.
A schema change invalidates compiled rules and cursors. Adding a field/flavour
needs only its schema declaration; removing or renaming one referenced by a rule
produces an error, not a silent omission. Persisted binding versions protect
constraint meaning from implicit changes on a Mara upgrade.

Validate definitions before conversion/evaluation: allowed keys and locations,
parameter types, name resolution, shape references, permitted combinations,
field datatype/endpoint compatibility and the existing recursion/depth limits.
Unknown keys such as `minCont` are `rule_invalid`, even if an RDF/SHACL library
would ignore them. Unsupported contexts and executable vocabulary are likewise
errors. Missing files, invalid definitions or engine errors cannot yield a pass.
`schema validate` checks the definitions without evaluating item conformance.

## Item projection and applicability

The internal namespaces are `urn:mara:rules:1:` for host selection metadata,
`urn:mara:flavour:` for flavours, `urn:mara:relation:` for canonical
relations and `urn:mara:field:` for custom fields. Authors need no prefix
declarations. These namespaces are part of binding version 1.

Project each internal item as `urn:mara:mid:MID`, with `rdf:type` for its
declared flavour. Project each canonical relation once, using its canonical
name as the predicate local part. Normalize inverse authoring first; SHACL
inverse paths choose incoming traversal. Symmetric edges expose both directions
but a self-edge appears once. Retain source occurrences outside the RDF set.
Flavour, relation and field names are encoded as UTF-8 percent-escaped IRI suffixes
when needed; do not normalize case or substitute alias names.

External endpoints remain distinct terminal nodes keyed by the exact normalized
address from [[DES-CANONICAL-TRACE-RELATIONS]], using
`urn:mara:external:` plus its percent-encoded address. They have no item flavour
or custom field triples. Count them where the relation permits; do not traverse
through them or interpret their URI as fetched RDF.

Project schema-declared custom fields as RDF literals under their field
predicates: strings/enums as `xsd:string`, booleans as `xsd:boolean`,
integers as `xsd:integer`, and finite numbers as `xsd:double`, using
the schema's parsed values. Reject unsupported numeric values rather than
coercing them. Each repeatable value contributes a triple; RDF has set
semantics, so order and duplicate occurrences remain in source metadata,
not field-count semantics. Absent optional fields contribute no triples;
an authored empty string remains a present literal. IDs, titles and relations
are not implicit custom fields. Invalid authored metadata makes the affected
item unavailable; it must not be projected as absence.

The host binding adds only these selection properties:

| Property | Allowed location and meaning |
|---|---|
| `whenShape` | At most one named, targetless NodeShape reference on an enabled root. Conforming means applicable; nonconforming means not applicable; omitted means applicable. |
| `paths` | Zero or more path/subtree strings on an enabled root, using existing retrieval path rules. OR within paths, AND with flavour targets. Omitted means all project paths. |

Internally these map to `urn:mara:rules:1:whenShape` and
`urn:mara:rules:1:paths`. Applicability and obligations both use ordinary
SHACL constraints. Condition shapes and their dependencies cannot carry host
selection properties; cycles are invalid. An unavailable condition yields
unavailable, never not applicable. A nonconforming condition produces no
`rule_failed`; only applicable rules evaluate their obligations.

A generic SHACL validator ignores host selection metadata. The adapter must
select applicable focus nodes before invoking obligation shapes. Reject
SPARQL/JavaScript constraints, external code, recursive references and unbounded
property paths in this initial profile.

Use `hasValue` for status conditions: absent status does not match.
Use `minCount: 1` for required presence; `datatype` and `pattern` alone
allow an empty value set. Schema enums constrain authored item values; a
well-formed literal condition outside an enum can simply never match.
Invalid source metadata is unavailable, not absence.

## Relationship shapes and evaluation

The initial SHACL profile supports flavour classes, predicate and inverse
relation paths, direct field paths, `sh:property`, `sh:node`, `sh:minCount`, `sh:maxCount`,
`sh:hasValue`, `sh:datatype`, `sh:pattern`,
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

Only schema-compatible relation/field paths are valid. An external-capable
path may use plain counts; field qualification must include a declared internal
flavour constraint. External nodes have neither that class nor item fields
and cannot qualify as internal items.
Unconstrained graph cycles do not change finite nested-shape semantics.
[[DES-TRACE-DIAGNOSTIC-INTERFACE]] defines depth and work limits.

Root severity is `sh:Violation` by default, mapped to error; explicit
`sh:Warning` maps to warning. The root owns the policy severity of its
reusable obligations. Nested severity overrides and other severities are
rejected initially. SHACL conformance and Mara validity are distinct:
a complete warning-only failure remains valid in Mara.

Preserve a separate evaluation-error ledger and authored-shape/source mapping.
A nonconforming qualifier is ordinary data; an engine error, invalid prerequisite
or exhausted budget must not be mistaken for a nonqualifying endpoint, even
when another endpoint satisfies the minimum. Evaluate only contexts selected
by root scope, applicability and explicit relationship obligations.

## Worked YAML rules

This complete file assumes optional project-defined statuses, an optional
string owner, and the directed schema relations shown by the paths.

```yaml
- id: rule:approved_requirement
  type: NodeShape
  targetClass: requirement
  whenShape: rule:approved_status
  property:
    - path: owner
      minCount: 1
      datatype: string
      pattern: '\S'
    - id: rule:verification_count
      path: {inversePath: 'schema:verifies'}
      qualifiedValueShape: rule:approved_verification
      qualifiedMinCount: 1

- id: rule:approved_status
  type: NodeShape
  property:
    - path: status
      hasValue: approved

- id: rule:approved_verification
  type: NodeShape
  class: verification
  node: rule:approved_status

- id: rule:accepted_design
  type: NodeShape
  targetClass: design
  whenShape: rule:accepted_status
  property:
    - path: 'schema:satisfies'
      qualifiedValueShape: {class: requirement}
      qualifiedMinCount: 1

- id: rule:accepted_status
  type: NodeShape
  property:
    - path: status
      hasValue: accepted

- id: rule:mitigated_risk
  type: NodeShape
  targetClass: risk
  whenShape: rule:mitigated_status
  property:
    - path: {inversePath: 'schema:mitigates'}
      qualifiedValueShape: {class: design}
      qualifiedMinCount: 1

- id: rule:mitigated_status
  type: NodeShape
  property:
    - path: status
      hasValue: mitigated
```

| Fixture | Required outcome |
|---|---|
| Approved requirement, owner present, only a draft verification | Failed: qualifying count 0, selected count 1. |
| Add an approved verification and repeat its authored edge | Passed: qualifying count 1, selected count 2. |
| Add `node: rule:approved_verification` to verification_count | Failed because of the draft; qualified minimum still passes. |
| No verification and only `node`, with no minimum | Passed. |
| No verification and minimum one | Failed minimum. |
| Approved requirement with blank/missing owner | Failed local obligation. |
| Draft requirement or absent optional status | Not applicable because the condition shape does not conform. |
| Accepted design without a satisfies edge | Failed; one requirement edge satisfies the minimum. |
| Mitigated risk without incoming mitigation | Failed; one design mitigation satisfies the minimum. |

To require passing evidence for each qualifying verification, append this
definition to the same sequence, or enable it in another configured YAML file:

```yaml
- id: rule:approved_verification
  property:
    - path: {inversePath: 'schema:evidences'}
      qualifiedValueShape:
        class: evidence
        property:
          - path: outcome
            hasValue: passed
      qualifiedMinCount: 1
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
reporting; ordinary constraint nonconformance still follows SHACL logic.

Ordinary source edits and structured mutations are evaluated identically;
policy failure does not introduce a new mutation gate. Rust library integration
is tested, but loading this persisted binding, precise source mapping and
enforcing whole-engine work accounting remain implementation obligations.

References: [SHACL](https://www.w3.org/TR/shacl/),
[JSON-LD contexts](https://www.w3.org/TR/json-ld11/#the-context) and
[scoped contexts](https://www.w3.org/TR/json-ld11/#scoped-contexts).
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
examination and graph-policy vertex/edge visit. String/collection operations
additionally charge their input byte/element counts before the operation;
pattern matching also requires bounded engine work, not just an input-length
charge. Schema validation charges each visited declaration and SHACL constraint.
Specification generation charges each source node/edge before fragmentation.
Charge logical visits even when cached. Applicability uses the same budget.
Counts alone at the SHACL invocation boundary are insufficient: enforce the
budget inside evaluation or report the operation unavailable. Do not
claim budget compliance from the successful library experiment alone.

Sort items by path/start byte, rules by expanded shape IRI, and relation edges
by canonical kind and endpoint identity. Order independent SHACL obligations
by source path/start byte; for repeated identical triples use the earliest
source location. RDF list members preserve list order. Blank-node labels from
a parser are not stable ordering keys. Pin the evaluator/cost-model revision
in the snapshot. Parallel scheduling must not change the
observable evaluated prefix. Loading/parsing source is outside this logical
evaluation budget and retains existing read/error behavior.

Allow at most eight nested relationship steps and SHACL shape-reference
depth 32 (root depth 1, including applicability dependencies). Reject deeper definitions, recursive shape
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
YAML rule locations identify the authored key/value span and structural pointer
when available. Retain those locations through generated JSON-LD/RDF nodes;
never report a generated context location as the authored rule source.
Only available coordinates are populated; never invent line numbers.
Retain legacy `path` and `line` fields as aliases of location coordinates.
An external configured schema path remains absolute.

Item diagnostics add `item:{id,mid}` when unambiguous; configuration/rule
diagnostics add `rule`, the expanded root shape IRI, when known.
Rule failures add `obligation:{shape,component,source}`: shape is its expanded
IRI or a snapshot-bound opaque reference for a blank node; component is the
SHACL component IRI; source is the authored YAML definition's location.
`details.kind` is `class|datatype|has_value|pattern|minimum|maximum|every|and|or|not|in`.
Local details identify the field path, constraint parameters and observed RDF
values or their bounded inspection references. Count details include
`selected_count`, `qualifying_count` and the violated bound.
Relationship explanations retain canonical relation, direction and
endpoint-facing label, plus item/edge references. Locations of all assertions
remain inspectable via relation get, not an unbounded inline list.

Emit one `rule_failed` per failed item/rule pair, pointing to the first
unsatisfied leaf in the obligation order above that contributes to the root failure.
Do not emit failures for unsuccessful alternatives of a passing SHACL or.
The matrix exposes the remaining check results, including all failed
alternatives when SHACL or fails. Unavailable rules produce
`evaluation_unavailable` referencing their prerequisite diagnostics or a
sanitized engine error instead of a fabricated policy failure.
An exhausted request emits one global
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
Schema validation also loads the configured YAML sources, checks the supported
SHACL/binding profile, generated vocabulary resolution, field datatype compatibility,
condition-shape references and graph policies, not runtime item conformance.
Unknown constraint keys fail before JSON-LD conversion can omit or reinterpret them.

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
| `trace matrix --flavour requirement --rule urn:mara:rule:approved_requirement` | `trace_matrix {flavours:["requirement"], rules:["urn:mara:rule:approved_requirement"]}` |
| `trace matrix --id REQ-A --check-file rules/coverage.yaml --shape urn:mara:rule:coverage` | `trace_matrix {ids:["REQ-A"], check:{files:["rules/coverage.yaml"], shape:"urn:mara:rule:coverage"}}` |
| `trace specification --path docs/` | `trace_specification {paths:["docs/"]}` |
| `trace specification --flavour requirement --field status=approved` | `trace_specification {flavours:["requirement"], fields:[{key:"status",value:"approved"}]}` |

Both accept `--all`, repeatable `--id`, `--flavour`, `--field`,
`--path`, and `--limit`, `--cursor`, `--max-work`.
Matrix additionally requires either repeatable `--rule` / nonempty `rules`,
or a request-local check, never both. Rule values are exact expanded root shape
IRIs from enabled sources; unknown IRIs are errors. Prefix abbreviations are
source syntax, not request aliases.

For a check, CLI accepts repeatable `--check-file` and one `--shape`;
MCP accepts `check:{files:[...],shape:IRI}`. Load those YAML sources with the generated bindings
using the rule-file contract and require the designated named node shape.
Apply it unconditionally to the request's selected roots; reject root targets,
`whenShape` and `paths` on the designated check, and do not execute other
targeted shapes from the supplied files. Its referenced obligations follow
the same field projection, profile and work limits as persisted rules.
The files define reusable constraints, not a saved view: selection stays in
the request. They impose no project-validation policy unless separately enabled
in project configuration.

Named rules retain their own selection and applicability, intersected with
view roots. Output distinguishes persisted-rule and request-check identity;
a request check cannot override a persisted rule. The former JSON `related`
check object is withdrawn with the custom predicate grammar. YAML expresses
SHACL shapes; it does not reinstate that predicate grammar.

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

`evaluation` is `{kind:"rule",shape:"urn:mara:rule:approved_requirement"}`
or `{kind:"check",shape:"urn:mara:rule:coverage"}`. `root` and item endpoints use discovery item descriptors;
external endpoints use the relationship contract's external descriptor.
Every record carries `kind`. `condition` identifies the SHACL component and
its parameters. Nested shape obligations have their own check records.
A check's `counts` contains `selected`,
`qualifying`, `minimum` and `maximum`; omitted bounds and unavailable totals
are null. `every` and `qualification` use the predicate states or null when
that test is not requested. `context` is an ordered array of canonical edges.

An endpoint context is the root plus the ordered canonical edges to that
check; at most eight hops. `check` references are snapshot-bound opaque
identifiers, not durable item identities. They connect records across pages.
Counts belong to the immediate check: include selected and qualifying totals,
declared minimum/maximum, and every state where present. Counts are null
when unavailable, not misleading zeros. Local checks retain field paths,
constraint parameters and bounded value/source inspection references.

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
| Schema | Keep the planned format 3 for vocabulary/relationships and structural cardinality/acyclic declarations. Conditional rules are separate YAML shape files, not an embedded rules mapping in the vocabulary schema. Absent policies impose no obligations. |
| Documents | No new marker or metadata syntax. Status and other rule inputs are ordinary project-defined fields. |
| Project configuration | Continue accepting format 1 for projects without rule sources. Enabling YAML rule sources requires format 2 and the optional rules table in DES-TRACE-RULE-GRAMMAR; reject unknown/unsupported versions. No saved views or persisted work limits. |
| Rule binding | Start format_version 1 inside the rules table. It selects the supported YAML/SHACL Core profile, generated context and namespaces, field projection and host selection vocabulary. This is independent of W3C or crate release numbers. |
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
relationship compatibility contract. Without rule sources, no project-config
migration is required beyond the relationship baseline. To enable rules, declare any needed custom fields,
create and review YAML shapes, change project
`format_version` to 2, and add `[rules]` with binding version 1 and explicit
file paths. Preserve existing project/content configuration. Validate source
loading and compare policy output before and after. Do not populate
statuses, owners, evidence, or links automatically.

Example: a customized schema with optional requirement status migrates to 3
without making drafts invalid. Adding the approved-requirement rule then
reports missing owners/approved verifications only for approved requirements.
Changing its severity to warning preserves predicate results while allowing
a complete project with only those failures to remain valid.
Malformed YAML, unsupported bindings and invalid shape definitions prevent
successful adoption. Absent optional status does not satisfy a status
`hasValue` condition; invalid authored enum values remain field errors.
Presence constraints are explicit, as specified in [[DES-TRACE-RULE-GRAMMAR]].
Restore the checkpoint or correct failed declarations, never report a
completed migration.
Preserve all IDs/MIDs and unrelated source bytes. New migration automation and
broader vocabulary transformations remain separate implementation/design work.

## Future transition extension exercise

Current-state rules always inspect the supplied current snapshot, even when
run after a direct Markdown edit. They do not imply that an item moved through
allowed states, had prior approval, or has fresh evidence.

Future transition policy needs a separately versioned binding that explicitly
projects previous/current snapshots. Its applicability shape would require
previous status draft and current status approved; its obligation shape would
require a nonblank current owner. These are the same kinds of Core field
constraints used today, over a future snapshot projection.

The current binding exposes only the supplied snapshot. Previous/current
projection vocabulary is unsupported and must be rejected. Do not infer a
baseline or silently reinterpret current-state field paths.

A future comparator would pair the same MID in two explicit revisions;
renaming its human ID would not create a different identity. For a draft to
approved pair the future obligation requires current owner; unchanged
approved to approved does not activate this transition rule, while the existing
approved_requirement current-state rule still activates.
Missing previous/current items, added/deleted items, baseline provenance and
evidence freshness need a later contract. Do not interpret a missing previous
snapshot as a successful transition or add executable transition support in
0.3. This exercise follows [[ADR-CURRENT-STATE-BEFORE-TRANSITIONS]].

The earlier custom predicate grammar and Turtle binding were never shipped.
The YAML shape profile replaces both; it is not a compatibility interpreter
for the old predicates. Do not add automatic conversion, Turtle input/export
or JSON-LD input surfaces. There are no authored context files to migrate.

Generated bindings follow the rule-binding version and project schema.
Changes to field/flavour names require corresponding rule edits; schema changes
invalidate compiled shapes and cursors. Namespace or implicit coercion changes
require an explicit compatibility decision. Existing item documents, MIDs and
active format-2 schema files remain unchanged during this documentation-only
adoption. A future expansion of SHACL support or projection likewise requires
a compatibility decision.
:::

:::mara decision ADR-DECLARATIVE-TRACE-BASELINE
:mid: 01M2JP4QMA7C4Z6H382XC5AFFG
:title: Keep trace policy explicit and distinguish incomplete evaluation
:justifies: DES-TRACE-RULE-GRAMMAR
:justifies: DES-TRACE-GRAPH-CONSTRAINTS
:justifies: DES-TRACE-DIAGNOSTIC-INTERFACE
:justifies: DES-TRACE-VIEW-INTERFACES
:justifies: DES-TRACE-CONTRACT-COMPATIBILITY

Adopt SHACL Core for local conditions and relationship obligations authored
only in YAML files referenced from project configuration. Mara generates the
fixed SHACL/datatype and schema-derived project bindings internally; users
maintain neither prefixes nor context files. The standard JSON-LD-to-RDF path
supplies the native SHACL engine, without a Turtle intermediate.

This supersedes the unshipped Turtle input and custom predicate grammar.
YAML improves authoring readability while retaining SHACL constraint semantics;
the generated context is a versioned adapter, not a separate expression engine.
There is no Turtle input/export or public JSON-LD format to maintain.
Other validators would require generated context/RDF, so independent consumption
of the authored YAML is not an initial capability. [[EVD-YAML-SHACL-SPIKE]]
verifies native Rust feasibility.

Keep the graph policies, diagnostic interface and request-selected views in
this document.

[[EVD-SHACL-CORE-SPIKE]] demonstrates the current worked obligations without a
second expression evaluator. If demonstrated rules exceed SHACL Core's
capabilities, reconsider integration with CEL; it is not a current dependency
or supported rule syntax. [[EVD-SHACL-CEL-SPIKE]] retains feasibility evidence.

The versioned Mara adapter owns item/field projection, applicability selection,
source mapping and completion reporting. A named ordinary Core condition
shape preserves not-applicable separately from passed; it adds a selection
reference, not a custom predicate grammar. Generic validators do not implement
Mara's selection metadata or incomplete-result semantics.

Absent fields contribute no RDF values. Presence requires an explicit minimum;
every over an empty related set passes. Combining every and a minimum supports
both optional-but-qualified and mandatory coverage.

Rules default to error and may be warnings. Invalid prerequisites and work
limits remain errors because treating an unperformed check as a policy warning
could claim a full pass. Keep complete evaluation distinct from output
pagination so small responses cannot hide incomplete analysis.

Keep one project-owned vocabulary schema, explicitly enabled YAML rule
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
