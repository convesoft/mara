# Reusable queries, policies and views

Selected engine work for [0.4.0-alpha.1](0.4.mara.md). These draft contracts
state the requested outcome, not implemented capability or passing evidence.
The existing installed Mara can continue evaluating this repository's current
rules while the new engine is developed.

## Authority and scope

The user selected a schema-independent query/rules engine: queries compute
facts; documentation rules and contextual gates judge them; views render them;
agents perform semantic reasoning. Engineering-template refinement is a separate
task. Do not change the self-hosted schema, its policies or the bundled engineering
template as part of this engine initiative. Fixture-owned definitions demonstrate
expressiveness without becoming product-wide engineering obligations.

Source discussions used these local design checkpoints under
`~/Documents/engineering/`:

- `mara-reusable-query-architecture-checkpoint.md`
- `mara-query-policy-gate-view-model-v1.md`
- `mara-query-catalog-gate-policies-v1.md`

Their assumed implementation status is not evidence. This document captures the
selected engine meaning so delivery does not depend on those private files.
Their ontology, risk thresholds and release policies are examples for fixture
verification, not adopted rules for every Mara project.

## Delivery boundary

Reuse the portable Rust engine and existing native/browser host boundaries.
External execution remains host-owned; the engine consumes explicitly supplied
facts and does not become a test runner, deployment integration or workflow
orchestrator. No UI, new host platform or provider plugin framework is included.

First specify the semantic input/result contracts, then prove the backend and
SHACL integration before fixing persisted syntax or broadly refactoring the
engine. The semantic contracts below fix observable behavior from the capability
contracts and the user's contract-authoring answers. The bounded proof selected
standard SELECT, a SHACL result bridge and the proof formats/operations/budget in
[[DES-QUERY-BACKEND-PROOF]]. The assigned implementation now carries that
selection through the named-query consumers and selected host interfaces; the
complete semantic catalog and integrated acceptance evidence remain separate.
Illustrative YAML or proposed commands outside the design do not select
interfaces. Do not build a custom query DSL or framework without explicit
approval.

The user subsequently approved [[DES-QUERY-NAMED-ADAPTER]] for the selected
named-query delivery. Its explicit input declarations, runtime output checks and
projected-support boundary govern wherever the earlier designs/catalog imply
automatic type/dependency inference or complete hidden derivation provenance.
Those stronger guarantees remain future intent, not this delivery's gates.
Other capability and host commitments remain; this is not a raw-query-console
pivot or evidence that the paused candidate passes.

Existing `trace matrix` is an obligation-evaluation explanation, not the
factual query/view abstraction. Its useful source-linked explanations inform
the new evaluation report; a matrix is a rendering, never an independent
traversal or policy mechanism.

## Capability contracts

:::mara scenario SCN-REUSE-QUERY-FACTS
:mid: 01M4BP7SNMXDHZ0T64JWETBD8G
:title: Reuse one graph question across independent consumers
:status: draft
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

A project author defines a named graph question and a caller selects roots and supplies context. Raw execution returns related bindings and lifecycle states, including roots with no matches. An agent inspects those facts, a view displays them, a documentation rule imposes an invariant, and a gate evaluates readiness for a supplied candidate. Each consumer uses the same query definition; an empty result has no inherent violation meaning. Missing required input is unavailable, not an empty successful result.
:::

:::mara requirement REQ-NAMED-QUERY-FACTS
:mid: 01M4BP86H18P09201BJZWZHH9V
:title: Execute reusable named queries as factual projections
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Support project/template-owned named queries with explicit root selection, parameters, forward/inverse relation traversal, filtering and structured projection. Definitions refer to the selected schema rather than hardcoded engineering flavours, fields, relationships or lifecycle values.

Return deterministic bindings and supporting source/relationship provenance, retaining selected roots with empty matches. Expose lifecycle values rather than implicitly restricting all queries to accepted items; explicit filters remain available. Distinguish valid empty results, invalid definitions or arguments, and unavailable/incomplete evaluation. Keep input identity and bounded continuation explicit. Query execution alone has no policy pass/fail semantics.

Definition resolution, explicit roots/parameters, typed bindings, absence, provenance and input/result completeness follow [[DES-QUERY-INPUT-RESULT-CONTRACT]], with the selected runtime-check/explicit-support boundary in [[DES-QUERY-NAMED-ADAPTER]].
:::

:::mara requirement REQ-QUERY-SINGLE-LANGUAGE-AUTHORING
:mid: 01M4JQXHMF6Z7P1CGK227J5W7H
:title: Author complete named queries in one existing language
:status: draft
:kind: constraint
:derives_from: SCN-REUSE-QUERY-FACTS

Author named-query inputs, output projections, computation and composition in one existing declarative language or an existing documented dialect. Do not require a separate hand-authored Mara-specific JSON definition or introduce a Mara-specific query/expression DSL. Generated metadata is not a competing authoring source.

Source: the user's query-authoring clarification: use one existing language to declare everything; GraphQL is not mandatory, and SPARQL is acceptable if a single query file covers the input/output declarations as well as the query.

Prefer existing syntax highlighting and project-aware autocompletion when selecting tooling. These are selection preferences, not yet an editor-integration acceptance gate. Standard SPARQL is now selected in [[ADR-QUERY-SPARQL-AUTHORING]]; the complete input/output and reusable-composition design still needs proof against the owning contracts. This requirement does not claim that the current JSON loader satisfies it.
:::

:::mara requirement REQ-COMPOSABLE-QUERY-PROJECTIONS
:mid: 01M4BP8KCRXSB29201PHSAYZA6
:title: Compose graph facts and derived projections without domain code
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Allow named queries to compose through joins, unions, explicit transitive traversal and derived projections. Support counts, existence and all/any/none conditions, deterministic text normalization/comparison, result aggregation with definition-selected precedence, and caller/project-supplied lookup mappings. Preserve supporting bindings instead of reducing every derived result to a boolean.

Make traversal direction, duplicate/counting behavior, cycle termination and evaluation limits explicit. A transitive ancestry query expands context without silently expanding caller-selected workload or walking back down into siblings. No matches, a missing execution result, and an unavailable computation remain distinguishable. Engineering-specific formulas, lifecycle selection, matrices and thresholds belong in definitions/inputs, not Rust branches.

Distinct binding/counting, nonrecursive composition, seed-excluding ancestry, finite cyclic reachability, limit outcomes and explicit aggregation/mapping behavior follow [[DES-QUERY-COMPOSITION-CONTRACT]].
:::

:::mara requirement REQ-EXPLICIT-QUERY-INPUTS
:mid: 01M4BP904M6HT15VPWQZ056VQD
:title: Consume host-supplied facts and context without owning execution
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Evaluate queries over durable graph knowledge and explicitly supplied external fact snapshots, parameters and organization policy. Support caller-selected workload, candidate identities and opaque validation contexts without interpreting them as commands or fetching data implicitly. Retain provenance and input identity needed to explain the result and invalidate stale continuation or reused outcomes.

An absent result in a usable supplied snapshot differs from unavailable required input. Query execution does not run tests, authenticate reported evidence, persist transient results as documentation, or acquire deployment/provider credentials. Native code discovery remains host-owned; browser evaluation may consume supplied facts without introducing a browser code indexer.

Only absence within declared complete snapshot coverage is known missing; partial or out-of-coverage facts are unavailable. Coverage/content identity and typed-input boundaries follow [[DES-QUERY-INPUT-RESULT-CONTRACT]].
:::

:::mara requirement REQ-QUERY-POLICY-CONSUMERS
:mid: 01M4BP9D1A4MX4E078ZERNR202
:title: Separate documentation validity from contextual gate outcomes
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Documentation rules consume reusable query facts and impose enabled invariants contributing to project/item validation. Contextual gates consume queries plus explicit scope/context and report whether their declared conditions hold without changing documentation validity. Neither consumer duplicates the query's traversal definition.

Preserve applicability, severity where relevant, supporting bindings and source-linked explanations. Distinguish passed, failed, not applicable and unavailable outcomes; missing prerequisites cannot become a policy pass. Source conformance and schema graph constraints remain validation responsibilities independent of query policy. Gates report conditions, not authorization or automatic action execution. Agent semantic judgment remains outside deterministic enforcement.

Gates explicitly define empty-set behavior. Unknown applicability or unavailable required conditions yields overall unavailable, retaining known failures; documentation severity and validity remain separate under [[DES-QUERY-CONSUMER-CONTRACT]].
:::

:::mara requirement REQ-QUERY-VIEWS-AND-REPORTS
:mid: 01M4BP9SY141XDMXE827KVFK4W
:title: Render factual views separately from policy explanations
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Provide factual tables/relationship matrices over query results and explanation reports over documentation-rule/gate outcomes. Views preserve bindings, relevant source links, input identity and completeness without selecting different graph facts or inventing obligations. Compose queries/projections before rendering; presentation is not a separate semantic mechanism.

A root with no related matches can appear in a factual view without being marked failed. An evaluation report identifies the obligation, outcome and qualifying or missing support. Reuse useful existing trace explanation behavior without treating the current trace-matrix operation as the new factual query API.

Factual rendering and evaluation explanations preserve the owning result semantics under [[DES-QUERY-CONSUMER-CONTRACT]]; rendering cannot repair or reinterpret incomplete facts.
:::

:::mara requirement REQ-QUERY-ENGINE-SURFACES
:mid: 01M4BPA6PXXQA31XTGSHVZVXFH
:title: Expose shared query and policy semantics across supported consumers
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Expose raw named-query execution, documentation-rule validation, contextual gates and factual/evaluation rendering through the shared Rust engine and the selected 0.4 native and browser application paths: embedded/daemon Rust, CLI/MCP, Node binding, shared TypeScript client, daemon HTTP and worker/WASM. Use one semantic implementation, with typed input/result contracts and consistent errors and continuation.

Hosts load or supply the required sources/facts; adapters do not independently traverse the graph or reinterpret policy. Honor existing host capability boundaries and report unavailable required inputs explicitly. Verify real native and browser execution rather than treating a compile or mock adapter as semantic parity.

Selected consumer coverage and unavailable-capability behavior follow [[DES-QUERY-CONSUMER-CONTRACT]]; typed meaning is shared even while concrete public formats remain pending proof.
:::

:::mara design DES-QUERY-POLICY-ENGINE
:mid: 01M4BPAKHAB32ZDSSNSG9EQSFR
:title: Separate factual evaluation, policy consumers and rendering
:status: draft
:kind: structure
:satisfies: REQ-NAMED-QUERY-FACTS
:satisfies: REQ-COMPOSABLE-QUERY-PROJECTIONS
:satisfies: REQ-EXPLICIT-QUERY-INPUTS
:satisfies: REQ-QUERY-POLICY-CONSUMERS
:satisfies: REQ-QUERY-VIEWS-AND-REPORTS
:satisfies: REQ-QUERY-ENGINE-SURFACES

Within mara-engine, share canonical graph preparation, typed values, normalized
relationship identities and source mappings. Named query evaluation produces
facts/bindings with provenance and completeness. Documentation rules and
contextual gates consume those results; renderers consume facts or evaluations.
Native hosts and browser workspaces supply inputs through
[[DES-EXECUTION-BOUNDARIES]].

The semantic handoff is defined by:
- [[DES-QUERY-INPUT-RESULT-CONTRACT]]: definition/schema resolution, roots,
  parameters, typed bindings, provenance, external coverage and input identity.
- [[DES-QUERY-COMPOSITION-CONTRACT]]: distinctness, joins/unions, transitive
  traversal, cycles/limits, logic, aggregations and policy mappings.
- [[DES-QUERY-CONSUMER-CONTRACT]]: documentation validity, contextual gates,
  applicability/severity, views/reports and selected host coverage.

Keep schema vocabulary and engineering semantics in project/template
definitions. External fact acquisition belongs to the host; no provider framework
is required. Do not reconstruct complete query results from SHACL evaluation
caches, whose observations reflect only evaluated constraints.

The bounded backend proof and selected standard SELECT/result-bridge contract
are owned by [[DES-QUERY-BACKEND-PROOF]], with rationale in
[[ADR-QUERY-SPARQL-RESULT-BRIDGE]]. Direct execution and SHACL consume the same
definition; SHACL receives the returned distinct tuples, not the source graph.
The three semantic contracts still govern the complete engine. The initial raw
named operation/formats live in [[DES-QUERY-INPUT-RESULT-CONTRACT]] and reuse
canonical relation/scalar preparation with the proven SELECT backend. Proof
format 1 remains distinct from named execution and does not complete the
policy/gate/view operations or final host coverage.
The bounded subset and subsequent integrated fixture checks are distinguished
in [[VER-REUSABLE-QUERY-ENGINE]]. A required backend conflict remains blocked
pending a user decision, not silent reduction or a custom DSL/framework.

These semantic designs retain full-engine intent; [[DES-QUERY-NAMED-ADAPTER]]
explicitly defers their automatic analysis and hidden-support guarantees for the
selected delivery. The bounded backend/bridge subset does not certify complete
new operations or host parity. Engineering-template refinement and
self-hosted schema/policy changes remain outside this engine initiative.
:::


:::mara design DES-QUERY-INPUT-RESULT-CONTRACT
:mid: 01M4BZ3CY8E7GJ9317QWDZP7ZF
:title: Bind reusable query facts to explicit typed inputs
:status: draft
:kind: interface
:satisfies: REQ-NAMED-QUERY-FACTS
:satisfies: REQ-EXPLICIT-QUERY-INPUTS

Semantic contract for the selected 0.4 query engine, with the initial raw named
operation specified below. Composition/external inputs and final consumers
remain separate delivery work. The contract-authoring user clarification selected
strict identity, selection and typed-input behavior.

### Definitions and schema binding

A caller/host loads an explicit project/template definition set for one project
and selects an exact definition identity. Identities are unique across that set;
duplicates, unresolved references and recursive dependencies are definition
errors. No load-order precedence, implicit template override, fuzzy lookup,
network import or cross-project resolution applies. Identity spelling, namespace
encoding and persisted definition syntax remain for the backend proof.
References bind to the selected definition content, not merely its name.

Bind declared flavour, field, relation and inverse-alias references against the
selected schema before execution; unknown declaration vocabulary is a
source-linked definition error. The richer intent also detects incompatible
endpoint/value types and composition signatures before execution. Automatic
proof of every possible type/signature is deferred under
[[DES-QUERY-NAMED-ADAPTER]]: validate actual returned values instead.
Normalize aliases and symmetric edges using the canonical relation model;
built-in and schema relationships remain distinguishable. Lifecycle values
are schema/project data, not implicit accepted-only filters. A schema or source
snapshot that cannot establish the needed graph is unavailable input, not a
successful graph with omitted nodes. Query facts do not require enabled policy
to pass before they can be inspected; an invariant violation is not missing input.

### Invocation and selection

An invocation identifies its definition, source/schema snapshot, root selector,
typed parameter bindings, required external snapshots and policy mappings, plus
evaluation/continuation controls. Select item roots explicitly by exact ID/MID
references, schema-bound filters or explicit all; omission never means all.
Within a filter category alternatives are OR, while different categories
intersect. Explicit all is exclusive of filters/references. Normalize aliases
to item identity and select a root once; an unknown or ambiguous explicit item
reference is an argument error, never silently dropped. A valid filter matching
nothing is a complete empty selection. Context traversal does not add roots.

Definitions declare parameter names, types, requiredness and any defaults.
Missing required, undeclared, duplicate or wrong-type bindings are argument
errors; no implicit text-to-number/bool conversion. Defaults apply only when
declared, not to an explicitly supplied empty value. Composition passes declared
parameters explicitly and checks signatures. Candidate identities and validation
contexts are opaque exact values, not Git refs to resolve or commands to run.

### Typed results and provenance

A result distinguishes item references, canonical edges, code endpoints,
external addresses, schema-typed scalars and derived structured bindings.
Item identity uses MID where present, otherwise the project-scoped human ID;
ID and MID references to the same item join as one identity. Code references
retain exact discovered descriptors; no name/position fallback. External
addresses retain the canonical graph's address identity and are never fetched.
Text, integer, number and boolean do not join by stringification or implicit
cross-type conversion. Preserve lifecycle and other field values as data.
Structured projections declare named typed columns/collections; incompatible
values are errors, not silently coerced or discarded.

Selected roots have explicit records even when they have no matches. A root
with zero matches is not a synthetic matched row, null entity or policy failure.
Optional missing fields/left-join bindings are absence, distinguishable from
empty text, false, zero and unavailable computation. Required missing external
records follow coverage below, not optional-field semantics.

For the selected adapter, support means explicitly projected references under
[[DES-QUERY-NAMED-ADAPTER]], not automatically recovered hidden contributors.
The richer hidden-derivation intent is deferred: each returned fact/derived value
would retain its root, contributing typed bindings, definition/derivation
references and source support: document locations,
canonical relationship endpoints and authored occurrences, or supplied external
snapshot/record provenance. Aggregates keep support rather than only a boolean.
Use finite supporting edges/derivations, not enumeration of infinitely many
cyclic walks. Order roots, bindings and provenance deterministically for identical
inputs; hosts and pagination must not change logical equality, counts or order.
The proof selects concrete ordering/encoding without losing these distinctions.

### External coverage, identity and completeness

External snapshots declare identity, typed records and the scope for which they
claim complete coverage, including relevant subjects, candidates and validation
contexts. A complete snapshot with no record for a covered key establishes known
missing support. Missing snapshots, loading failures, partial coverage and
out-of-coverage keys are unavailable for that computation. Malformed supplied
facts/type conflicts are invalid inputs; conflicting records cannot silently
overwrite one another. Uniqueness keys follow the definition's fact signature.

Mara validates declared identity/coverage compatibility, not the truth,
authenticity or freshness of reported execution. It never runs the reported
test, acquires credentials or persists transient facts as documentation.
Code discovery remains native-host-owned under [[DES-EXECUTION-BOUNDARIES]].

Bind results and reusable outcomes to source inventory/content and selection,
schema, the selected definition and its transitive dependency content, resolved
roots/parameters, external data/coverage/identity, mappings, candidate/context and
semantic evaluation controls. Opaque labels alone cannot hide changed content.
Changes invalidate continuation/reuse even when an output row happens to match.
Identity is not evidence authentication.

Distinguish definition/argument errors, complete facts (including emptiness),
and unavailable/incomplete evaluation with reasons and any known partial support.
Completeness covers the logical evaluation; page completion covers delivery.
A page boundary is not a traversal limit. Summaries/counts describe the full
logical result, never only the current page. Cursors bind the evaluation identity
and unchanged request, preventing mixed snapshots, repeated/lost rows or stale
success. [[DES-QUERY-BACKEND-PROOF]] selects the bounded proof's format/operations and
identity encoding. Complete definition-set, input and continuation encodings
must implement this contract during the later full-engine work.

### Named-query operation

`SourceEngine::query_execute(named_query::Request)` is shared by embedded and
daemon `NativeRuntime`, CLI/MCP, the Node addon, daemon HTTP, the shared
TypeScript client and worker/WASM. CLI example:
`mara --execution-mode direct --format json query execute ID --definition-file queries.json --id ROOT --max-rows 100 --limit 20`.
MCP `query_execute` accepts the same request plus optional project selection.
Every adapter exposes its backend's persistence and indexing capabilities; it
does not invent missing code indexes or fetch external facts.

#### Definition formats 3, 4 and 5

Format 5 adds host-supplied external fact inputs, typed lookup mappings and
definition-selected precedence aggregation. Its concrete declarations, request
inputs, coverage/missing semantics, derived outputs and implemented Rust/CLI/MCP
and browser-worker slice are specified in [[DES-QUERY-FACT-COMPUTATION]].
Formats 3, 4 and 5 are accepted by raw queries, documentation-rule
validation, gates, views and reports on the selected adapters. The format-4
composition rules below also apply to format 5.

Project-relative JSON files contain `{format_version:3,definitions:[...]}`, `{format_version:4,definitions:[...]}` or `{format_version:5,definitions:[...]}`. Format 3 remains the single-query form;
format 4 adds only explicit named dependencies; format 5 adds the external-fact,
lookup and aggregation declarations of [[DES-QUERY-FACT-COMPUTATION]]. Format-2
files are rejected because they do not declare required sources.
Each definition declares exact `id`, standard `select`, nonempty
`root_flavours`, `required_fact_sources` and named `columns`. `documents` is a
required fact source; `code` is recognized but unavailable to this embedded
snapshot, so requiring it produces an incomplete result even for empty roots.
Required sources are explicit, unique, and never inferred from query variables,
columns or possible paths. Unknown sources are invalid declarations. The
`external` source requires declared external fact inputs.

Columns declare `type`, optional item `flavours`, and `optional` (default
false). Types: item, code, external, iri, text, integer, number, boolean, edge.
Null is optional absence, not empty text, false or zero. Text retains optional
language. Numbers carry canonical finite double value text; shared scalar
preparation preserves numeric values rather than source lexical spellings, with
both zero signs representing zero.

Optional `parameters` maps names to `{type,required,default?}`.
Request bindings are `[{name,value:{type,value}}]`, not an overwriting map.
Input types: text, integer, finite number, boolean, exact item reference, IRI.
Defaults are typed; explicit empty text never invokes a default. Duplicate,
unused, missing required and wrong-type declarations/bindings fail.
CLI example: `--param 'state={"type":"text","value":"draft"}'`.

Optional `source_columns` project extra canonical item references;
`support_columns` project canonical edge references excluded from tuple identity.
These are the only support/contributor sources in this adapter; hidden query
contributors and derivation reconstruction are not promised. Resolve each
projected reference through the shared graph, and retain authored occurrences
for projected edges. SELECT projects exactly root and declared tuple/source/
support columns. Parameter variables bind separately. Every definition in the
explicitly loaded set is parsed, profile-checked and schema-bound before root
enumeration, including unused definitions and empty selections; only the
selected definition and its reachable dependencies execute. This checks
declaration vocabulary and projection shape, not possible result types. Check declared
value types, item flavours and canonical endpoints only for values actually
returned; a no-match query need not reveal an incompatibility.

Format 4 dependencies are metadata, not an operation DSL. Each dependency names
an exact definition in the explicitly loaded files, binds `root` to the caller's
selected item, optionally maps child parameter names to parent parameters, and
explicitly projects child tuple columns, contributors and/or edge support under
local aliases. The `projections` object is required; `{}` exposes only the
child-row existence. Validate the entire loaded dependency graph before root
enumeration, including unreachable definitions. Dependencies must be acyclic;
unresolved references, invalid mappings and recursive graphs are definition
errors. Only reachable definitions execute. A child runs for the same
selected root, with only its declared parameters. Returned child bindings are
materialized as scoped in-memory relations. Parent joins, unions, filters,
optional matches and property paths use ordinary admitted SPARQL. In the
parent query, `<urn:mara:query:input:NAME>` links the selected root to a child
row; `<urn:mara:query:column:NAME:ALIAS>` reads a declared projected value.
Undeclared dependency predicates are rejected. Child result types are checked
when returned; automatic full type inference is not provided. Only projected
child support is propagated, with canonical edges and their authored
occurrences; hidden derivations are not reconstructed.

Typed result tuples are distinct by their declared values, independent of
support. Duplicate rows from `UNION`, aliases or multiple paths merge support
and contributors. A parent projection that merges rows retains their combined
explicit support. Inner joins require a match; explicit `OPTIONAL` preserves a
matched left row with absent values. No child match does not remove the selected
root record. Summary counts count distinct completed bindings, not assertions
or paths; page size does not change counts. The observed row budget counts
solution rows across child and parent evaluations. If required work reaches the
budget, the result is incomplete; already observed bindings and their projected
support remain available, but the exact count and negative conclusions are
unavailable.

The named adapter orders projected solution terms in Oxigraph before applying
the observed-row budget, so partial prefixes do not depend on native/WASM store
iteration. Lexical, language and datatype tie breaks distinguish RDF terms that
compare equal numerically. The input identity binds this ordering revision,
rejecting earlier continuations. Shared discovery/occurrence selectors also use
fixed-width content framing across native and WASM. This internal execution ordering does not admit
caller `ORDER BY`. The row budget does not bound backend sorting, graph work,
time or memory.

For exact deterministic text comparisons, definitions may query
`<urn:mara:query:nfc-casefold:title>` or
`<urn:mara:query:nfc-casefold:field:FIELD>`. These expose stored values normalized by NFC and full default Unicode case
folding; original titles, fields and support remain unchanged. Literal query
operands must already use the same normalized form. This is exact equality
after normalization, not fuzzy or locale-sensitive matching. Field predicates
require a declared string/enum field.

The admitted standard SELECT profile follows [[DES-QUERY-BACKEND-PROOF]]
with the named-only numeric extension below. Bind flavour/field/relation URNs
to the selected schema; inverse aliases normalize canonical direction, including
property paths. Structural ID/title predicates are `urn:mara:item:id` and
`urn:mara:item:title`, distinct from custom fields. Ordinary predicates must
be schema-bound constants. Query-specific additions are the exact dependency,
normalized-text, format-5 fact-input and occurrence predicates specified here;
existing canonical edge-support predicates remain available. Open
negated-vocabulary paths and nested SELECT boundaries are not this delivery's
composition interface. No SERVICE, imports or updates are admitted. Outside the
expressly admitted numeric operators, implicit numeric/boolean conversion is
not admitted.
Initial admission limits do not remove later composition/derived-value commitments.

For the integrated catalog, the user approved a bounded named-query extension:
standard SPARQL multiplication and ordering (`*`, `<`, `<=`, `>`, `>=`)
execute in Oxigraph. The format-1 backend proof profile remains unchanged.
Exact typed equality still uses `sameTerm`; no numeric-coercing `=` or
second expression evaluator is added. Returned values retain runtime type
checks. Definitions own formulas and thresholds; typed lookup inputs supply
factors through explicitly projected dependency columns.

`urn:mara:query:occurrence` links a canonical edge to each finite authored
occurrence IRI, `urn:mara:query:occurrence:<identity>`. Its identity is the
existing content-bound identity in projected edge support. A definition can
select distinct occurrences and count them with format-5 aggregation; duplicate
paths do not multiply an occurrence. Project the edge as support to retain
its authored locations. This exposes existing source facts, not hidden path
derivations or automatic provenance reconstruction.

#### Request and factual page format 3

A request supplies `definition_files`, exact `definition`,
`selection:{all?,ids?,flavours?,fields?:[{key,value}],paths?}`, typed
`parameters`, positive u32 `max_rows`, `limit` in 1..=100 and optional
`cursor`. No selector is an argument error; all excludes selectors.
Selected items must satisfy the declared root signature. Traversal never expands
roots. Seed-excluding ancestry uses an explicit
`FILTER(!sameTerm(?root,?ancestor))` with `parent+`; include-self is an
explicit different question.

Format-3 pages identify definition/file, SHA-256 input_identity, logical
evaluation_complete/unavailable_reason, full-result summary, ordered records,
has_more and next_cursor. Records are root headers (identity/source/count)
and distinct bindings tied to a root. Empty roots have a header, no binding.
OPTIONAL does not manufacture a left-side match where the rest of the pattern
had none; when the left side matches, absent projected values remain a binding.
A supported all-absent projection remains one distinct binding. Bindings retain only explicitly projected source items and canonical edges
with their authored occurrences. Unprojected joins, UNION/OPTIONAL matches,
EXISTS filters and path derivations do not add hidden support. Complete negative
filters bind the definition and complete source snapshot, not invented edge
occurrences.

Order roots by canonical item IRI, tuples by serialized typed values, and
contributors/edges by stable content keys. MID identity wins; absent MID uses
project-scoped ID identity without manufacturing a MID. Missing MID alone does
not hide facts; ambiguous/malformed identities or graph prerequisites remain
unavailable. Documentation's MID requirement and graph policies are unchanged.

Definitions/arguments fail with source-linked invalid_definition/invalid_argument.
Missing definition/schema/document prerequisites and backend/row-budget failure
are unavailable/incomplete, not empty success. Native failures to construct a
snapshot use input_unavailable. Hosts accept caller-supplied, typed external
fact snapshots and lookup mappings; they do not fetch or persist them. A
signature requiring unavailable code-index facts reports incomplete rather
than returning empty success. Browser/WASM does not add native code indexing;
durable external-address edges remain typed factual values.

Summary selected_roots/known_bindings cover the logical result; binding_count and
root counts are null when incomplete. Delivery is at most limit records and
65536 serialized bytes per page. An indivisible oversized record errors rather
than disappearing or continuing forever. The observed solution-row budget
includes child and parent solution rows, including duplicate support rows, not
graph-work/deadline/memory guarantees.

Continuation recomputes the question and binds normalized source content/
inventory/ignore context, schema/configuration, every loaded definition source,
original selection/typed bindings, row budget, page limit and application/result
version. Changes reject old cursors even when rows would match. There is no
persisted result cache. Native calls reload explicit query files; reused sessions
cannot conceal definition changes. Consume every page with unchanged inputs.
:::

:::mara design DES-QUERY-COMPOSITION-CONTRACT
:mid: 01M4BZ3SV8DTX6ZV7XVY8270R8
:title: Compose distinct bindings and finite graph reachability
:status: draft
:kind: behavior
:satisfies: REQ-COMPOSABLE-QUERY-PROJECTIONS

Semantic composition consumes [[DES-QUERY-INPUT-RESULT-CONTRACT]]. The user selected versioned dependency metadata and explicit child-input/projection bindings, not a general composition-operation tree or custom query DSL. Existing standard SPARQL remains the query language.

### Selected definition format

Format 4 definitions may name exact child definitions from the explicitly loaded definition files. Each dependency binds the child root to the caller-selected `root`, maps any child parameters to declared parent parameters, and declares which child tuple columns, contributors and edge support are projected under local aliases. The `projections` object is required; `{}` exposes only child-row existence. The `projections` object is required; `{}` exposes only child-row existence. Validate the entire explicitly loaded dependency graph, including unused definitions; unresolved references, invalid bindings and recursion are definition errors. Only reachable definitions execute. Format 3 remains valid for definitions without dependencies.

A child runs for the same caller-selected root. Its typed bindings become scoped in-memory relation rows. Parent joins, unions, filters, optional matches and property paths are expressed in the admitted standard SPARQL profile, using only the dependency's declared `urn:mara:query:input:<name>` and `urn:mara:query:column:<name>:<alias>` predicates. No implicit root expansion or parameter inheritance is allowed. Validation completes before root enumeration, including for an empty root selection. Check returned child and parent values against declared types; full static dependency/type inference remains deferred.

Only explicitly projected child support propagates. Canonical edge identity and its authored occurrences are retained when projected; hidden derivation reconstruction is not promised. Joins use compatible returned RDF terms and exact typed identity, not titles or formatted cells. The parent may project less than the child; if that makes output tuples identical, merge them and combine their explicit support/contributors.

### Composition and equality

Named-query dependencies form an acyclic graph. Resolve reachable dependencies before execution and reject direct/indirect recursion. Preserve strict validation of every loaded definition's SELECT, schema vocabulary and projection shape, even when unused; only reachable definitions execute. Automatic proof of every compatible type/signature remains richer future intent under [[DES-QUERY-NAMED-ADAPTER]]; returned values are checked. Compose results through joins on explicit compatible typed keys, unions of compatible projections, filters and derived projections. Inner joins retain matching bindings; explicit left joins retain a matched left binding with absent values when the right side has no match. They do not manufacture a row when the left side has no match. Neither removes the selected-root records when its match set is empty. A composed child receives explicitly bound roots/context/parameters; ambient project-wide roots and implicit workload expansion are not allowed.

Results use distinct typed binding tuples by default. Repeated authored assertions, alias spellings or multiple supporting paths do not multiply an identical binding. Combine its supporting provenance. Projection may merge bindings that become identical after columns are removed, retaining their combined support. For the selected adapter this is explicitly projected support, not reconstruction of hidden operands or paths. Union uses the same distinctness. Joins match typed identity, not titles or formatted cells; support from both operands remains attached.

A definition specifies the counted binding/column and qualification. Ordinary counts count distinct tuples/endpoints, not authored assertions or paths. Explicit assertion-occurrence counting counts distinct authored occurrence identities, even if the same occurrence is encountered by two paths; pagination never changes counts. Unbounded walk/path counting is not part of this contract.

### Traversal

Definitions choose relation kinds, direction, qualification and direct versus transitive expansion. Ancestry expands only the declared upward relation; ancestor context and its related needs/goals/risk support do not become workload, and reverse descent into siblings is not implicit. External addresses are terminal unless explicitly joined to supplied fact records; URLs are not graphs to dereference.

Transitive reachability returns each qualifying reachable endpoint once. Ancestry excludes its starting root, including when a nonzero-length cycle returns to it; include-self must be explicit. Standard property-path evaluation terminates on finite cyclic graphs while retaining only finite, explicitly projected support. Cycle presence does not itself invalidate facts: project schema acyclicity remains [[REQ-RELATION-CYCLE-POLICY]]'s independent validation obligation. A constrained cycle can therefore be inspectable facts and a documentation violation, not a query-engine permission to relax the schema.

If an evaluation resource limit stops required expansion/composition, mark it incomplete/unavailable, retain any known support with the limit reason, and never infer a complete negative, exact count or policy pass from truncation. The current observed-solution-row budget spans child and parent evaluations; it is not a graph-work or deadline guarantee. A definition's intentional bounded-hop question is different: fully evaluating that declared domain can be complete. [[DES-QUERY-BACKEND-PROOF]] defines the proof's observed-row budget and its lack of cooperative traversal-work cancellation. Complete composition and continuation must preserve these limit outcomes; no timing guarantee is selected.

### Derived values, logic and mappings

Equality/text comparison is exact and type-aware by default. Definitions may explicitly compare the exposed `urn:mara:query:nfc-casefold:title` or schema string/enum-field predicates. Values use deterministic Unicode NFC normalization and full default (nonlocale) case folding; preserve original text and support. Do not add fuzzy matching or locale-dependent behavior. The embedded adapter normalizes stored item values. Literal query operands must already use NFC plus full default case-folded form; no SPARQL normalization function is added.

Existence and any on a complete empty set are false; all and none are true; a distinct count is zero. These are query facts, not readiness judgments. Missing required input cannot be converted into any of those empty-set values. Unknown operands remain explicit; a predicate may be known when complete available support logically settles it, but unavailable required dependencies remain visible and completeness is not upgraded. Gates follow [[DES-QUERY-CONSUMER-CONTRACT]], including explicit empty-set interpretation.

Definitions select aggregate/grouping keys, qualification and result precedence; there is no built-in ordering of pass/fail/inconclusive/error/pending, lifecycle rank or risk formula. An aggregate declares its empty case and retains members and support. Conflicting or underspecified precedence is a definition error, not load-order selection.

Lookup mappings are typed caller/project policy data bound into input identity. Validate key/value types and reject conflicting duplicate keys. A missing key is unavailable unless an explicit fallback is declared; it never silently maps to zero, low risk, pass or no match. Fixture-defined likelihood/consequence scores, thresholds and reassessment use these mechanisms. Implementation links do not automatically lower a risk or select a new assessment. The concrete format-5 declarations, coverage overlay, mapping and aggregate semantics are in [[DES-QUERY-FACT-COMPUTATION]].
:::

:::mara design DES-QUERY-CONSUMER-CONTRACT
:mid: 01M4BZ46QQPRXCCWF85EJRS7V1
:title: Separate query-backed validity gates and presentation
:status: draft
:kind: behavior
:satisfies: REQ-QUERY-POLICY-CONSUMERS
:satisfies: REQ-QUERY-VIEWS-AND-REPORTS
:satisfies: REQ-QUERY-ENGINE-SURFACES

Consumers reuse the same definition-bound facts under
[[DES-QUERY-INPUT-RESULT-CONTRACT]] and [[DES-QUERY-COMPOSITION-CONTRACT]].
Neither policies nor renderers independently reconstruct traversal.

### Policy outcomes

A documentation rule declares its query references/bindings, target,
applicability, predicate, severity and source-linked obligation identity.
Only explicitly enabled invariants contribute to project/item validation.
Source/schema conformance and declared graph constraints remain independent.
A contextual gate declares the same evaluation inputs plus explicit
workload/candidate/context and is request-local judgment, not enabled
documentation policy or authorization to execute an action.

For each applicable condition report passed, failed or unavailable with
support/reasons. A known false applicability condition yields not_applicable,
not failed; unknown applicability yields unavailable, not an exemption.
Inputs required only by an inapplicable branch need not be evaluated.
A gate must explicitly declare how empty root selections and empty required
sets affect it (satisfied, failed or not applicable); missing such semantics is
a definition error. This does not change mathematical raw-query empty facts.

A completed applicable evaluation is passed only when all its declared required
conditions hold; known false conditions fail. A gate with any unavailable
required condition/applicability has overall unavailable and completeness=false,
even alongside known failures. Retain failed and unavailable check records;
do not hide a defect behind the summary. When no applicable required conditions
remain, apply the gate's declared empty-required-set outcome; individual
inapplicable conditions still report not_applicable.
Definitions decide which results are required and how domain states map into
conditions; an informational/unused input is not automatically a prerequisite.

Error-severity documentation violations make validation invalid; warning-only
violations remain valid when evaluation is complete, following existing
validation conventions. Unavailable enabled validation remains incomplete and
cannot establish valid documentation. Gate failure, warning severity or
unavailability never changes documentation validity by itself. Severity is
reported separately from factual truth and applicability; a warning is not
rewritten into passed. Invalid definitions/arguments are diagnostics/errors,
not ordinary false facts or not_applicable outcomes.

### Implemented query-consumer slice

The shared consumers reuse named-query formats 3, 4 and 5 and the existing
projected-result SHACL bridge. They do not add traversal or validation behavior
to engineering rules or templates. `SourceEngine`, `NativeRuntime` and
`NativeClient` expose documentation-rule validation, request-local gates,
factual views and evaluation reports. The same operations are wired through
CLI/MCP, daemon IPC/HTTP, Node, the shared TypeScript client and worker/WASM.
Caller-supplied facts and mappings flow through evaluation and input identity;
missing required input remains unavailable, including on an empty scope.

Project configuration enables validation invariants with:

```toml
[query_policy]
format_version = 1
files = ["invariants.yaml"]
definition_files = ["queries.json"]
shape_files = ["policy.ttl"]
max_rows = 100
```

Each YAML policy file has `format_version: 1` and `invariants`. An invariant
requires `id`, exact named `definition`, exact SHACL `shape` IRI, nonempty
`flavours` within the definition's root signature, and `severity` (`error` or
`warning`); `paths` and typed `parameters` are optional. It runs for selected
project roots or the selected item only. Each root's query result is checked
independently. Validation returns optional `query_policies` format 1 with
`input_identity`, `evaluation_complete`, and checks containing rule, definition,
shape, root, state, applicability, severity, exact binding count, SHACL
components, typed bindings/support and policy-source location. Validation pages
contain diagnostics first, then policy checks; `query_policies.checks` is
page-local while policy completeness and validation summaries cover the full
target. The unchanged validation cursor continues across both lists. A completed
failed invariant adds `rule_failed` at its authored severity. Missing or
incomplete required inputs add `evaluation_unavailable` and cannot pass.
Schema validation checks policy definitions/shapes without evaluating facts.
Source conformance and structural graph constraints remain separate checks.

A gate file has `format_version: 1` and uniquely selected `gates`. Every gate
requires explicit `empty_roots` and `empty_required` outcomes (`passed`, `failed`
or `not_applicable`), plus `conditions`. Each condition identifies a named
`definition`, its explicitly supplied `shape_file` and exact `shape` IRI,
`severity`, and optional exact `context_parameters` and `applies_when`
(`bindings_present` or `bindings_absent`). A root outside a definition's declared
root flavours is not applicable; context needed only by that inactive condition
is not required. Other missing or unused context bindings are invalid.

`QueryGateRequest` requires explicit root `scope` (`all` or one or more exact
ID/flavour/field/path filters), typed context bindings and positive `max_rows`.
Its format-1 result reports each root and condition's applicability, severity,
state, count, SHACL component, typed supporting bindings and source locations,
plus the input identity and evaluation completeness. `shape_file` selects the
condition's shape graph; duplicate shape IRIs and blank-node labels in other
supplied files do not alter that condition. A complete empty scope is classified
only after source-conformance prerequisites pass; unavailable required query
sources keep the gate unavailable even when the selected root set is empty. For
a complete empty scope, `empty_roots`
determines the result and `empty_reason` is `roots`. For a nonempty scope with no
applicable required conditions, `empty_required` determines the root/aggregate
outcome and `empty_reason` is `required_conditions`. Otherwise any unavailable
required condition makes the aggregate unavailable and incomplete while retaining
known failed condition records; absent unavailable conditions, a known failure
makes it failed. Gate outcomes are condition reports only: they neither change
`ValidationResult.valid` nor authorize or execute an action.

`query gate` and `query report gate` accept the gate identity, `--gate-file`,
repeatable `--definition-file` and `--shape-file`, explicit root selectors
(`--all`, `--id`, `--flavour`, `--field KEY=VALUE`, `--path`), typed
`--param NAME=JSON`, repeatable `--fact-file`/`--mapping-file` and
`--max-rows`. MCP `query_gate` accepts the corresponding typed snapshots and
mappings; conditions that fail are successful result data, not tool errors.
`item_validate`, `project_validate` and the documentation-rule report accept
the same host inputs. Named query definitions,
gate YAML, policy YAML and SHACL Turtle bytes participate in the source identity.
Blank-node labels remain local to each supplied Turtle file when shapes are
combined; equal authored labels in separate files never merge constraints. The
focused cross-consumer regression is
`tests/named_query.rs::shared_query_facts_become_invariant_and_gate_failures_across_rust_cli_and_mcp`.
It proves one complete empty fact result remains informational, fails an enabled
error/warning invariant as authored, fails a separately requested gate, retains
a known gate failure beside an unavailable condition, and applies both explicit
empty-gate cases. The companion host tests exercise the same supplied inputs through native
daemon IPC and HTTP, Node, CLI/MCP and browser worker paths.

Reports identify the rule/gate, definition and obligation sources, selected root,
applicability/severity, candidate/context identity, each condition outcome and
qualifying, missing or unavailable support. Overall completeness and known
failures remain visible even when output is paged or filtered.

### Views and host coverage

Factual tables and relationship matrices consume query bindings. Preserve
empty roots, original typed values, supporting links, input identity and logical
completeness. Presentation may hide/reorder columns or format labels, but must
not change query membership, counts, conditions or gate results. A display label
does not become a comparison key. Evaluation reports explain policy outcomes;
a factual matrix has no implicit violation cells or readiness thresholds.
The existing trace matrix is an explanation reference, not the new query API.

The selected 0.4 shared operations cover raw query execution, enabled
documentation-rule validation, contextual gates, factual views and evaluation
reports across embedded Rust, daemon Rust, CLI/MCP, N-API/Node, the shared
TypeScript client, daemon HTTP and worker/WASM. Loading/transport/persistence
differences follow [[DES-EXECUTION-BOUNDARIES]] and [0.4 coverage](0.4.mara.md).
Required code/external facts unavailable on a host produce explicit unavailable
results, not a host-specific empty success or an invented browser indexer.
For identical supplied semantic inputs, hosts share outcomes, bindings, identity
and completeness; they do not interpret policy independently.

### Implemented view and report slice

Factual views and evaluation reports consume the same composed results;
renderers are pure and never re-run a traversal or reinterpret a policy
outcome. `mara-engine`'s `query_views` module defines both formats;
`SourceEngine::query_view` composes one named-query page and
`SourceEngine::query_report` composes a gate or enabled documentation rules.
`NativeRuntime`/`NativeClient` expose both through CLI/MCP, daemon IPC/HTTP,
Node, the shared TypeScript client and worker/WASM. MCP uses `query_view` and
`query_report` (`subject.kind` is `gate` or `documentation_rules`). Both accept
`render: "markdown"`; CLI human output uses the same renderer and `--format json`
returns the structured format.

Factual view format 1 (`kind: "query_view"`) preserves the page's `definition`,
`definition_file`, `input_identity`, `evaluation_complete`, `unavailable_reason`,
`summary` (`selected_roots`, `binding_count`, `known_bindings`), `has_more` and
`next_cursor`. It adds declared `columns` (name, type, optional, flavours) from
the selected definition and `roots` in page order with unchanged typed `rows`
(`values`, `contributors`, `support`, and any explicitly projected `facts`
provenance). A root record's `source` is linked;
bindings whose root record was on an earlier page keep their root endpoint with
`source` and `binding_count` absent rather than fabricating them. An empty root
is a complete empty result, never a failed cell. Literal text/number/IRI values
are escaped so their Markdown or HTML syntax renders as text; only generated
source links and emphasis are markup.

Evaluation report format 1 (`subject.kind` is `gate` or `documentation_rules`)
preserves the owning result's `input_identity` and `evaluation_complete`, the
gate aggregate `state`/`empty_reason`/`unavailable_reason` when present, and the
policy `diagnostics`. It also carries `roots`, one entry per evaluated gate root
with its outcome, independent of condition records, so a gate with no conditions
still preserves every selected root and its state. Each `checks` entry identifies
`obligation`, `kind` (`gate_condition` or `documentation_rule`), `definition`,
`shape`, optional `shape_file`, optional `root`, source-linked `source`,
`severity`, `state`, `applicability`, `support_state`, exact `binding_count`,
failed SHACL `components` and the unchanged typed `bindings`. `support_state` is
`qualifying` for a passed obligation, `missing` for a failed one, and
`not_applicable` or `unavailable` otherwise. A gate report grants no permission
and a rule report does not change documentation validity. The Markdown report
renders a gate's explicit scope and typed context, a roots/outcome table, then
each check's obligation, definition/shape source and root identity, so
candidate/context and multi-root outcomes stay attributable.

```sh
mara query view urn:fixture:query:supports --definition-file queries.json \
  --id SUB-LINKED --id SUB-EMPTY --param 'state={"type":"text","value":""}' \
  --max-rows 100 --limit 100 --cursor '<next_cursor>'
mara query report gate require-support --gate-file gate.yaml \
  --definition-file queries.json --shape-file presence.ttl --id SUB-EMPTY \
  --param 'state={"type":"text","value":"accepted"}' --max-rows 100
mara query report rules
```

The consumer regressions exercise real Rust/CLI/MCP paths, with companion
fixtures for native daemon IPC/HTTP, installed Node and browser worker/WASM.
`tests/node/usage.mts` compile-checks the TypeScript request/result surface.
[[VER-REUSABLE-QUERY-ENGINE]] and the final supported-host gates own the
matching-artifact and integrated workflow evidence; source or test declarations
alone do not establish a pass. [[DES-QUERY-BACKEND-PROOF]] selects the backend
and actual proof formats/operations, and demonstrates the SHACL result bridge.
:::

:::mara decision ADR-QUERY-SPARQL-AUTHORING
:mid: 01M4JRWKGZ7QP77JVXJQXPMXNZ
:title: Use standard SPARQL as the named-query authoring language
:status: draft
:justifies: REQ-QUERY-SINGLE-LANGUAGE-AUTHORING

The user selected standard SPARQL as the named-query authoring direction after comparing GraphQL and the current JSON-plus-SPARQL definitions. Keep query selection, computation and composition in standard SPARQL files; do not require a hand-authored Mara JSON wrapper or invent query directives, decorators or another expression language. Generate tooling metadata from the project schema and query structure where justified.

Rationale: reuse an existing query language and its syntax highlighting/completion ecosystem rather than maintaining a Mara-specific authoring syntax. GraphQL and Rego are not selected alternatives for this change.

This selects the language, not a completed replacement format. Standard SPARQL does not define cross-file stored-query imports or typed function declarations; the input/output and reusable-composition design must preserve the owning contracts without pretending private extensions are standard. Invocation, source loading, completeness and provenance remain explicit Mara execution responsibilities. Do not claim full static inference or successful replacement from language expressiveness alone; verify the real catalog and selected hosts before adoption. The current JSON loader remains unchanged.
:::

:::mara decision ADR-QUERY-EXACT-RESOLUTION
:mid: 01M4BZ7NS7R4DV5SJ0H74M1J91
:title: Reject ambiguous query identities and lenient invocation
:status: draft
:justifies: DES-QUERY-INPUT-RESULT-CONTRACT

The user selected unique identities in an explicitly loaded project/template
definition set rather than automatic project overrides or same-name namespace
resolution. Strict explicit roots and declared typed arguments avoid silently
changing the question, dropping misspelled roots or coercing context. Exact
typed comparison remains the default; text normalization is explicit.
[[DES-QUERY-INPUT-RESULT-CONTRACT]] owns the behavior. Identity encoding and
persisted syntax still require the backend proof; this choice does not select a
language. Revisit only through an explicit contract change if the proof cannot
support it.
:::

:::mara decision ADR-QUERY-DISTINCT-SUPPORT
:mid: 01M4BZ82PBDDEE9FHC9TDDV7ZP
:title: Count distinct facts rather than repeated graph support
:status: draft
:justifies: DES-QUERY-COMPOSITION-CONTRACT

The user selected distinct typed bindings with combined provenance rather than
match/path multiplicity. Two routes to one supporting design or repeated
assertions must not inflate ordinary coverage. Explicit authored-occurrence
counting remains available when assertions themselves are the subject.
[[DES-QUERY-COMPOSITION-CONTRACT]] owns equality/counting and finite support;
no unbounded walk counting or automatic bag semantics is selected.
:::

:::mara decision ADR-QUERY-FINITE-COMPOSITION
:mid: 01M4BZ8FRQENDGTNV7CM7D4QX4
:title: Use explicit reachability without recursive query definitions
:status: draft
:justifies: DES-QUERY-COMPOSITION-CONTRACT

The user selected acyclic named-query dependencies rather than recursive
fixed-point composition. Explicit transitive reachability still supports graph
cycles where schema policy permits them; it does not relax declared acyclicity.
Ancestry excludes its seed, and resource truncation means incomplete/unavailable,
not successful absence. [[DES-QUERY-COMPOSITION-CONTRACT]] owns these semantics.
This bounds the backend proof without dropping transitive graph questions;
numeric budgets and implementation mechanisms remain proof work.
:::

:::mara decision ADR-QUERY-DECLARED-FACT-COVERAGE
:mid: 01M4BZ8WQ4QF9JENV5P7HMV024
:title: Require complete declared coverage to establish absent external facts
:status: draft
:justifies: DES-QUERY-INPUT-RESULT-CONTRACT

The user selected explicit complete snapshot coverage rather than assuming any
supplied snapshot covers every subject/candidate/context. Absence within coverage
is known missing; absence outside it is unknown/unavailable. A partial result
file must not establish that a verification was not run. Identity/coverage
checks do not authenticate host evidence or run external tests.
[[DES-QUERY-INPUT-RESULT-CONTRACT]] owns this boundary.
:::

:::mara decision ADR-QUERY-EXPLICIT-POLICY-OUTCOMES
:mid: 01M4BZ99MYTAGJF0KC7VHWF0T6
:title: Keep gate emptiness and unavailable outcomes explicit
:status: draft
:justifies: DES-QUERY-CONSUMER-CONTRACT

The user selected gate-defined empty-selection/required-set interpretation
rather than automatic vacuous readiness or universal empty failure. Raw complete
empty facts retain mathematical all/any/none semantics. For a gate with both a
known required failure and an unavailable required check, overall unavailable
and incomplete takes precedence while the failed check remains visible.
This distinguishes completed negative evaluation from an unfinished one.
[[DES-QUERY-CONSUMER-CONTRACT]] owns outcomes; domain aggregation precedence and
mapping fallbacks remain explicit definition/policy data under
[[DES-QUERY-COMPOSITION-CONTRACT]], not engine defaults.
:::

:::mara verification VER-REUSABLE-QUERY-ENGINE
:mid: 01M4BPB0C5MZA2W617BTPW6MHP
:title: Demonstrate reusable query, policy and view capabilities on real hosts
:status: draft
:method: test
:level: system
:verifies: REQ-NAMED-QUERY-FACTS
:verifies: REQ-COMPOSABLE-QUERY-PROJECTIONS
:verifies: REQ-EXPLICIT-QUERY-INPUTS
:verifies: REQ-QUERY-POLICY-CONSUMERS
:verifies: REQ-QUERY-VIEWS-AND-REPORTS
:verifies: REQ-QUERY-ENGINE-SURFACES
:verifies: DES-QUERY-POLICY-ENGINE
:verifies: DES-QUERY-INPUT-RESULT-CONTRACT
:verifies: DES-QUERY-COMPOSITION-CONTRACT
:verifies: DES-QUERY-CONSUMER-CONTRACT

Use disposable fixture projects with fixture-owned vocabulary, definitions,
source knowledge and external fact snapshots. Keep synthetic assertions out of
the self-hosted corpus. Exercise actual engine/application paths; supplied
runtime results are inputs, not evidence that Mara ran external tests.
The expectations below discriminate implementations of
[[DES-QUERY-INPUT-RESULT-CONTRACT]], [[DES-QUERY-COMPOSITION-CONTRACT]] and
[[DES-QUERY-CONSUMER-CONTRACT]]. They are future execution checks, not passing
evidence from contract authoring. Fixture labels are not public operation names,
persisted syntax or an adopted engineering ontology.

### Selected named-query adapter gate

Apply [[DES-QUERY-NAMED-ADAPTER]] before interpreting the catalog below. For the
first raw-query stage, exercise actual Rust and embedded CLI/MCP with a
project-owned SPARQL definition: explicit/empty roots, typed parameters,
lifecycle-bearing returned values, optional absence, and projected canonical
support resolved to authored sources. Check invalid declarations/arguments,
a returned wrong-type value, declared unavailable inputs and a document-only
query that does not require undeclared code facts. A no-match query need not
expose every potential type incompatibility. Compare deterministic paged facts
and reject continuation after relevant definition/source/parameter changes.
Hidden derivation reconstruction and full static type/dependency inference are
not acceptance gates. Final formats must state this support boundary.

Raw-stage success does not certify the broader semantic catalog or consumer
parity. The assigned consumer/host slice carries the adapter boundary through
policy, gates, views, reports and the selected transports; its integration proof
remains part of [[VER-REUSABLE-QUERY-ENGINE]]. These assertions do not adopt the
deferred automatic inference or hidden-provenance guarantees. Preserve actual
prior failing reviews; only revised, executed checks and required review establish
acceptance.

### Discriminating fixtures

1. **One question, four consumers.** Select a linked root and an empty root.
   The query returns draft/accepted/retired supporting items without implicit
   lifecycle filtering and retains the empty root. Reuse the exact definition
   for raw facts, factual rendering, an enabled documentation rule and a
   candidate gate. A fixture invariant requiring one accepted link fails only
   for the uncovered root; the raw empty facts and view do not themselves fail.
   With that invariant disabled, structurally valid but uncovered documentation
   is valid while a separately requested gate requiring coverage fails.
   A rendering does not alter either evaluation.

2. **Resolution and arguments.** Two loaded definitions sharing an identity,
   an unresolved/recursive query reference and an unknown schema name in a
   declaration are definition errors. Under the selected adapter, returned values
   incompatible with declared types are errors; a relation/join producing no rows
   need not reveal all potential incompatibilities before execution. Missing/
   wrong-type/undeclared parameters and unknown explicit item IDs are argument errors.
   No override, implicit all or dropped IDs is allowed. Select the same root by
   ID and MID: one root record. A valid filter matching nothing gives complete
   empty selection, while missing source inventory cannot establish emptiness.

3. **Typed values and collisions.** Distinguish text `"1"`, integer `1`,
   boolean true, empty text and an absent optional field. Exact typed joins do
   not conflate them. Two fixture titles/aliases such as `Straße` and `STRASSE`
   collide only when explicit full default case folding is selected.
   Composed/decomposed Unicode spellings match under explicit NFC normalization,
   not an implicit fuzzy/default comparison. Preserve original text and sources.
   Basis alternatives, inverse verification links, decomposition and
   supersession expose lifecycle-bearing supporting bindings.

4. **Distinct support and joins.** Two paths reach the same supporting design;
   repeated metadata/inline/inverse assertions denote the same edge. Endpoint
   count is one and all explicitly projected canonical support remains inspectable.
   Explicit assertion-occurrence count distinguishes two authored assertions
   without counting one occurrence twice through different paths.
   Union/projection deduplicate identical tuples; an explicit left join retains
   absence and the root, whereas an inner join has no matching tuple.
   Full-result counts do not change with page size.

5. **Alternative implementation support.** A fixture requirement has direct
   implementation support, or support through an accepted satisfying design.
   The fixture definition accepts either and returns the qualifying path and
   bindings. Removing the design must not break the direct alternative.
   No design-mandatory obligation or lifecycle value is hardcoded in Rust.

6. **Workload versus ancestry.** Workload contains only child C. Parent links
   C→A, C→B, A→G and B→G form a diamond; sibling S also links to A. Return
   A/B/G once each plus declared need/goal/risk context; never add ancestors
   or S to workload or walk down into S. With an unconstrained cycle C→A→C,
   traversal terminates and excludes C from its ancestry. An explicit include-
   self projection contains C once. A separate fixture schema prohibiting that
   cycle fails documentation validation, not generic traversal.
   A deliberately reached evaluation limit gives incomplete/unavailable and no
   exact truncated count or gate pass; fully evaluated intentional hop bounds
   can be complete. Distinguish output continuation from resource truncation.

7. **Candidate verification and coverage.** The fixture gate explicitly selects
   every accepted verification required for candidate C and requires a nonempty
   set with passing results. Complete coverage containing passes for V1/V2
   passes; a complete C snapshot missing V2 fails with known missing support;
   failed V2 fails with its record. A partial V1-only snapshot, a snapshot for D,
   or unavailable input produces unavailable for C, not missing or passed.
   No accepted verification definitions yields the declared empty-required-set
   failure, not vacuous readiness. A missing required named query definition is
   a definition error, not that empty-set case. Nonaccepted verifications are
   excluded only by the fixture's explicit filter.

8. **Context aggregation.** Supply pass/fail/inconclusive/error/pending records
   for two validation contexts. A fixture explicitly requires only one context
   and chooses domain precedence error > fail > inconclusive > pending > pass.
   Verify the selected members and aggregate, ignoring records in the other
   context without treating them as matching evidence. Replace the definition's
   precedence and observe the corresponding changed aggregate, not a Rust
   default. No record in complete required-context coverage is known missing;
   partial/out-of-context coverage is unavailable.

9. **Replaceable risk policy.** A fixture explicitly computes likelihood ×
   consequence and judges scores >= 6 above its threshold. Supplied mappings
   likelihood=2 and consequence=3 yield 6; replacing likelihood with 1 yields 3
   on explicit reevaluation. Preserve the selected mapping/input identity and
   supporting risk bindings. An unmapped key is unavailable; an explicitly
   configured fallback produces its declared value. Conflicting duplicate
   mapping keys are invalid. Adding implementation does not automatically
   lower the existing risk or create a reassessment.

10. **Applicability, severity and incomplete gates.** Known false applicability
    is not_applicable without requiring facts used only by the inactive branch;
    unavailable applicability is unavailable, not exempt. Query all/any/none
    on complete empty sets yields true/false/true. Gates with declared empty
    pass/fail/not-applicable cases return those cases; an unspecified gate empty
    case is invalid. A known failed required check plus an unavailable required
    check yields overall unavailable, completeness=false, retaining both.
    A completed error-severity documentation violation makes validation invalid;
    warning-only complete violations preserve validity and report failed warning
    checks. Gate failure alone never changes documentation validity.

11. **Provenance and identity.** Distinguish a known missing covered fact from
    a missing snapshot; link known external records to their snapshot/record
    provenance and graph support to authored occurrences. Page the same frozen
    inputs without losing/repeating bindings, roots or explanation support.
    Changes to source inventory/content, schema, a referenced definition,
    parameters/roots, fact content/coverage, mappings or candidate/context reject
    old continuation and reused outcomes. Keeping an opaque identity label
    unchanged cannot conceal changed content. View formatting changes do not
    alter factual membership or condition outcomes.

12. **Same semantics on selected hosts.** Run identical semantic fixture inputs
    through raw queries, enabled rule validation, gates, factual views and
    evaluation reports on real embedded/daemon Rust, CLI/MCP, installed
    Node/shared TypeScript and Chromium HTTP/worker-WASM paths. Compare typed
    bindings, support, outcomes, identities, completeness and relevant errors,
    not only formatted cells or successful transport. Missing required native
    code facts on a document-only browser path is explicitly unavailable;
    supplied supported facts do not imply a browser indexer.

### Proof and integrated execution gates

Before broad implementation, the bounded backend proof executes the same query
definition directly and through the SHACL bridge, demonstrating inverse traversal,
lifecycle bindings, empty roots, distinct qualified counts and source-linked
explanations natively and in real browser WASM. It must preserve the selected
contracts; unsupported semantics require a user decision. The selected backend, bounded profile, persisted proof definition/result versions,
actual proof operations, row budget and limitations are recorded in
[[DES-QUERY-BACKEND-PROOF]]. Its Rust/Playwright fixture demonstrates this subset;
it does not substitute for the later complete catalog. No custom DSL/framework
is authorized by this verification.

The complete fixture catalog and selected host parity remain integrated release
checks. `tests/fixtures/query-catalog/snapshot.json` owns reusable catalog
sources/definitions. `tests/query_catalog.rs` asserts the twelve discriminators
through portable and direct/daemon native Rust, CLI and MCP. Its optional
`MARA_QUERY_CATALOG_REPORT` captures executed inputs and complete outcomes for
the installed Node and Chromium HTTP/worker catalog tests. Those tests compare
owning typed results and errors, including identity, support and completeness;
a skipped opt-in proof is not acceptance. Receipts remain outside the corpus.
This implementation also includes focused native Rust/CLI/MCP consumer tests,
real daemon IPC/HTTP tests, installed Node operation tests, compile-checked
TypeScript usage and browser worker/WASM tests. These tests exercise the selected
paths but do not replace the matching-artifact integrated proof in
[[VER-REUSABLE-QUERY-ENGINE]]. Use the existing supported-host gates, with Apple
Silicon execution at the final host gate. Record actual tested revisions,
artifacts, environments, commands, results and limitations. A fixture
demonstration is not adoption of the engineering template, authentication of
reported external evidence or proof of external execution.
:::

:::mara design DES-QUERY-BACKEND-PROOF
:mid: 01M4D4F67Y1QVKAG5E4H6H7W3J
:title: Execute standard SELECT facts through a SHACL result bridge
:status: draft
:kind: interface
:satisfies: REQ-NAMED-QUERY-FACTS
:satisfies: REQ-QUERY-POLICY-CONSUMERS
:satisfies: REQ-QUERY-ENGINE-SURFACES

Selected after the bounded native/Chromium worker proof for [[DES-QUERY-POLICY-ENGINE]]. This design fixes the backend and proof interface, not completion of the full query engine.

### Backend and language

Use unmodified Oxigraph 0.5.11 in-memory SELECT execution and spargebra 0.4.7 parsing, with existing shacl/rudof_rdf/rudof_iri 0.3.24 for constraint evaluation and OxRDF 0.3.4's standard RDFC-1.0/SHA-256 canonicalization for anonymous policy identities. Disable Oxigraph default features. Its native HTTP handler is explicitly disabled; browser builds have no HTTP handler. Reject SERVICE (including SILENT), dataset loading and updates before execution. Shared typed/canonical RDF preparation is reused by existing rules and queries; hosts supply every source.

The executable proof profile admits SELECT, basic graph patterns, forward/inverse/property paths, joins, OPTIONAL, UNION, MINUS, VALUES, projection and DISTINCT. Expressions admit RDF terms, variables, BOUND, sameTerm, boolean and/or/not and EXISTS. Do not admit SPARQL '=' as Mara's exact typed comparison: it performs cross-datatype numeric equality. ORDER BY, REDUCED, slicing, grouping/aggregates and other functions are rejected in this bounded proof, not substituted with empty results. RDF-star and directional RDF-1.2 literals are rejected; feature closure does not broaden the RDF-1.1 result profile. Validate the SELECT signature before root enumeration, including empty selections. Admitted query/policy literals are strings/language-tagged strings and canonical primitive xsd:integer (i64), finite xsd:double (excluding negative zero), and xsd:boolean spellings. Integer/double spellings must equal i64/f64 Display (as used by pinned oxsdatatypes), and boolean spelling must be true/false; this is backend primitive spelling, not XML Schema's canonical scientific notation. Reject other datatypes/facets, noncanonical spellings and nonfinite values: Oxigraph normalizes known numeric forms before sameTerm/projection, so accepting them would lose literal identity. Source scalar results use the shared typed projection and backend canonical primitive spelling; this is not a guarantee for arbitrary RDF literal lexical forms.

SPARQL is the selected standard query language, not a custom traversal DSL. Later engine stages add the owning resolution/coverage/composition checks and derived operations from the semantic designs, subject to the subsequently approved runtime-type/explicit-support boundary in [[DES-QUERY-NAMED-ADAPTER]]. The admission profile is a proof subset, not removal of other capabilities. Named composition can supply child projections as explicit in-memory relations; dependency order, declared signatures and projected source support remain engine responsibilities, not SHACL caches. Arbitrary SPARQL conformance is not promised. Numeric coercion, bag counts, SAMPLE/load-order precedence and backend errors must not redefine Mara's semantic contracts.

### Formats and operations

The portable proof uses JSON definition format 1: exact nonempty id, select text, columns naming tuple bindings and support_columns naming canonical edge support. ?root is the explicit invocation key, not a tuple column. Definition sources are supplied snapshot files; no imports/fetches or ontology in Rust. This metadata declares a standard SELECT signature; it is not an expression language.

Public embedded Rust methods and worker operations are query_backend_execute and query_backend_policy. The former takes definition_file, explicit roots and max_rows in 1..=4294967295 (u32 on both hosts); larger JSON integers are rejected before execution. The latter adds shape_file and exact shape IRI. Unknown explicit IDs fail; ID/MID aliases select one root. Unknown request fields and unsupported profile constructs fail. These bounded proof entry points remain distinct from the complete definition-set loader and consumer APIs exposed through CLI/MCP, Node, HTTP, the typed client and worker/WASM.

Factual result format 1 contains definition/file, SHA-256 input_identity, evaluation_complete, unavailable_reason and ordered root records. Each root has its item identity/source and a bindings array, empty when no matches. Tuple columns contain item identity/source, IRI or RDF literal (lexical value, datatype, optional language); an optional missing value is null, not an empty literal. An all-null projection of a real supported match remains one distinct tuple with combined edge support; only a row with neither projected values nor support is treated as unmatched. Literal datatypes distinguish integer, double, boolean and text; lifecycle is ordinary schema data.

Deduplicate typed tuples after removing support columns and combine canonical edge support. Each support record retains the edge and every authored occurrence/source. Edge IRIs are SHA-256 of canonical edge JSON. Portable query occurrence identities hash source snapshot, location, author, kind, relation and target; do not reuse native relation-inspection cursor hashes, which differ with host word width. Order roots by canonical item IRI, tuples by canonical serialized typed values and support by canonical edge key. Authored occurrence enumeration follows corpus source order. Full facts are independent of policy evaluation caches.

Input identity hashes the explicit snapshot fingerprint and request, including source/definition content and row budget. Proof calls return one complete delivery value and have no cursor; pagination and reusable outcome identity for later composition/inputs must follow [[DES-QUERY-INPUT-RESULT-CONTRACT]], not reuse this small request signature unchanged.

### SHACL result bridge

Execute the selected definition through the same factual method, then materialize only its distinct returned tuples under urn:mara:query:match and urn:mara:query:column:NAME. Each tuple is a blank row node, so row identity cannot collide with returned IRI values and fabricate property-path links. SHACL receives root/row/column RDF, not the original corpus; it cannot repeat the source traversal. The supplied shape chooses qualification and counts. The unchanged synchronous RuleEngine adapter retains nested-error detection and upstream literal/logical/qualification semantics. Counts refer to distinct projected tuples, not authored occurrences. Explicit occurrence-count questions must select distinct occurrence identities as their counted bindings in the later complete profile.

Evaluation format 1 includes the original facts and per-root state, exact policy source file/shape, failed constraint component IRIs and available total/qualifying counts. Canonicalize supplied policy RDF with RDFC-1.0/SHA-256 before compiling SHACL so anonymous shape IDs are stable for identical source inputs. Facts carry root/item locations and canonical authored support independently of the policy. Any incomplete facts suppress all proof-policy counts and produce unavailable checks; any SHACL adapter error makes evaluation incomplete. This is a request-local bridge demonstration, not enabled documentation validity or a complete gate implementation.

### Feasibility and limits

The proof executes inverse traversal, draft/accepted/retired fields, empty roots, ID/MID root deduplication, duplicate UNION branches yielding three distinct supports with five authored assertions, qualified counts, source-linked failed explanations, exact datatype distinctions and reached row limits through real native and Chromium worker/WASM artifacts. The same definition file is consumed directly and by SHACL. Focused regressions exercise supported all-null OPTIONAL projections, repeatable anonymous counts and empty-root signature/directional-literal rejection on both hosts, plus canonical string/integer/double/boolean distinction and sameTerm, rejected normalized literal facets/spellings, collision-free bridge rows and identical budget boundaries. The browser spec is opt-in when fixture/native-result inputs are supplied; missing inputs skip the bounded probe rather than break the existing default browser suite. Passing proof evidence requires actual execution, never that skip. Standard Rust tests and the existing Playwright worker harness compare complete factual/evaluation JSON.

max_rows bounds observed SELECT solution rows across selected roots, including duplicates that provide support. It is not page size, a wall-clock deadline, a bound on graph expansion before the next solution, or a memory/latency guarantee. A reached limit retains known partial support and every root, sets incomplete with a reason, and cannot establish an exact truncated count, complete negative or policy pass. No default numeric budget is implied; callers choose it explicitly. The synchronous upstream evaluator supplies no cooperative traversal-work cancellation hook; do not claim one.

The bounded proof requires document items with MIDs, one loaded definition and no external snapshots, typed parameters, mapping inputs, named dependencies, factual rendering or final host adapters. It does not certify the twelve integrated fixtures, arbitrary cyclic/path provenance, Unicode normalization, aggregate precedence, full coverage/type binding, installed packages or Apple Silicon. Later catalog/host work remains required by [[VER-REUSABLE-QUERY-ENGINE]] and the semantic designs, subject to the subsequently approved automatic-analysis/support deferrals in [[DES-QUERY-NAMED-ADAPTER]]. If implementation of a remaining required capability exposes a backend conflict, keep it blocked and seek a user decision; never silently widen SPARQL admission or reduce the contract. No template/schema/policy refinement or host effects are introduced.
:::

:::mara decision ADR-QUERY-SPARQL-RESULT-BRIDGE
:mid: 01M4D4FK9V9P2R9CHCXV57SXTZ
:title: Use in-memory standard queries and project facts into SHACL
:status: draft
:justifies: DES-QUERY-POLICY-ENGINE
:justifies: DES-QUERY-BACKEND-PROOF

The selected backend-proof/design-settlement work permits standard backend selection, not a custom DSL or reduced semantics. Native and real Chromium worker execution of one fixture SELECT establishes the bounded path in [[DES-QUERY-BACKEND-PROOF]].

Choose direct configured Oxigraph SELECT evaluation rather than rudof_rdf's optional QueryRDF convenience API: the latter silently returns empty solutions until ensure_store and does not expose evaluator SERVICE controls. Retain the pinned SHACL NativeEngine adapter for constraints, not facts. Materializing factual tuples as constraint input lets one definition serve raw and policy consumers without duplicating traversal or reconstructing facts from incomplete SHACL caches.

Existing canonical RDF preparation and schema-owned vocabulary avoid a second ontology. A direct parser/profile check rejects numeric-coercing equality and host-effect constructs. Reject affected literal forms that the backend normalizes rather than claiming arbitrary RDF lexical identity or inventing a custom codec; canonical Mara primitive comparisons remain the required path. Blank bridge rows prevent collisions with data IRIs, and a fixed u32 budget gives identical native/WASM JSON boundaries. Portable SHA-256 occurrence identity corrects the host-width-dependent relation handle observed in the first native/browser comparison without changing the existing relation API. Use upstream standard RDFC-1.0/SHA-256 to stabilize anonymous SHACL reporting rather than inventing a source parser or exposing random blank labels. Locked RDF-1.2 feature closure is required for compatible upstream RDF term enums; it does not authorize RDF-star output, directional literals or a broader public language.

This selects a backend and bounded proof contract, not proof of every planned query capability. At proof selection the semantic contracts remained unchanged; the subsequent [[ADR-QUERY-EXPLICIT-ANALYSIS-BOUNDARY]] narrows their automatic-analysis/support guarantees for the selected delivery. No traversal-work cancellation, arbitrary SPARQL support or completed host/catalog certification is inferred from this result. An unsupported required capability remains a blocker under [[VER-REUSABLE-QUERY-ENGINE]], not justification for custom language code or implicit scope reduction.
:::

:::mara decision ADR-QUERY-EXPLICIT-ANALYSIS-BOUNDARY
:mid: 01M4FGP9R5KBJMHKKA4G1XNZRZ
:title: Prefer explicit query dependencies and projected support to automatic analysis
:status: draft
:justifies: DES-QUERY-INPUT-RESULT-CONTRACT
:justifies: DES-QUERY-NAMED-ADAPTER

The user explicitly approved a typed named-query adapter around SPARQL after pausing the repeated correctness-review loop. Retain the product-facing API and safeguards rather than deliver a raw query console; let the existing standard backend own query semantics rather than continue a second automatic analysis engine.

Choose declared required fact sources, runtime checks of returned column values and explicitly projected provenance. The rejected alternatives for this delivery were continuing exhaustive pre-execution type/dependency inference and hidden-derivation reconstruction, or reducing the product to untyped raw SPARQL bindings.

The tradeoff is explicit: some type/endpoint incompatibilities surface only when a value is returned; no-match cases need not expose them. Source-linked support is accurate for returned references, not a claim of complete hidden derivation history. This narrows the earlier semantic designs for the selected delivery without removing their richer future intent or other scheduled capabilities.

Approval selects a contract, not a passing candidate, a review waiver, implementation resumption or integration/publication authority. Preserve prior failing reviews and work; assess and verify revised implementation against the changed obligations before acceptance.
:::

:::mara design DES-QUERY-NAMED-ADAPTER
:mid: 01M4FGPPRF05QKBSR0ZWH26EVN
:title: Adapt named SPARQL queries with explicit inputs and typed returned facts
:status: draft
:kind: interface
:satisfies: REQ-NAMED-QUERY-FACTS
:satisfies: REQ-EXPLICIT-QUERY-INPUTS

Selected by the user's approval of the typed named-query/SPARQL middle ground after the correctness-repair loop, with rationale in [[ADR-QUERY-EXPLICIT-ANALYSIS-BOUNDARY]]. This governs named-query results in the selected 0.4 delivery, including later consumers of those results. It narrows only automatic analysis/support guarantees in [[DES-QUERY-INPUT-RESULT-CONTRACT]] and [[DES-QUERY-COMPOSITION-CONTRACT]]; other capability and host commitments remain. Full static inference and hidden-derivation reconstruction remain future intent, not current acceptance.

### Definition and execution boundary

Load uniquely named project/template-owned definitions with native SPARQL SELECT sources. Small metadata declares identity, typed parameters, output columns, required fact sources and provenance columns; it does not encode query expressions. Concrete versioned file/request/result formats must be recorded with implementation, not inferred from illustrative syntax.

Validate declaration vocabulary/types against the selected schema, names and parameters, SELECT/profile syntax and declared column references before execution, including empty selections. Use the standard backend for admitted traversal, filtering, OPTIONAL and UNION semantics; do not add a second algebra/type-inference evaluator. [[DES-QUERY-BACKEND-PROOF]] retains the backend, host-effect restrictions and bounded proof profile; this selection does not promise arbitrary SPARQL support.

Required fact sources are explicit definition metadata, not inferred from possible variable types, hidden paths or output column kinds. Hosts supply those inputs. A declared required source that cannot be supplied is unavailable, not a successful empty result. Loading/malformed-input errors remain distinct. No implicit network imports, SERVICE, dataset acquisition or execution effects are introduced.

### Invocation and returned facts

Keep explicit roots, strict typed arguments/defaults and exact definition resolution under [[DES-QUERY-INPUT-RESULT-CONTRACT]]. An omitted selector never means all; ID/MID aliases select one root; invalid explicit roots are argument errors. Valid empty selections and roots with no matches remain explicit complete results when required inputs are available. Lifecycle is returned data, not an implicit accepted-only filter.

Declared output types are checked against actual returned values, including declared item flavours and canonical endpoint kinds. Wrong returned types/references are errors, never coerced, discarded or reported as empty success. Optional absence remains distinct from empty text, false and zero. There is no promise to prove every possible SPARQL variable type/flavour, endpoint compatibility or composition signature before execution, or to detect every incompatibility when no rows match.

Resolve returned item/edge/code references through shared canonical graph identities and source mappings. Definitions explicitly project the support references they want explained. Resolve and validate those references; canonical edge support includes its authored occurrences. Combine explicitly returned support when distinct typed tuples merge. Root/definition identity and source locations remain inspectable. Provenance describes projected support only: do not claim complete hidden contributors, filtered-out bindings or property-path derivations. More explanation requires explicit projections, not automatic reconstruction.

Keep deterministic root/tuple/support ordering, distinct typed bindings, logical completeness and bounded input-bound continuation. Changes to relevant source/schema/definition content, roots, parameters, declared fact inputs or evaluation controls invalidate prior continuation/reuse. A delivery page is not an evaluation limit; incomplete evaluation cannot establish complete negative facts or policy success.

### Selected workflow and limits

The initial raw-query stage preceded this assigned extension. This slice carries
format-5 supplied facts/composition through documentation policy, gates, factual
views and reports on the selected Rust, CLI/MCP, daemon, Node/TypeScript and
browser surfaces. Hosts retain fact acquisition; browser/WASM does not promise
native code indexing. The broader semantic catalog, full static inference and
hidden-derivation reconstruction remain outside this slice.

The user separately accepted current transitive-path preparation cost for this slice; optimization is not an acceptance prerequisite and no latency/workload guarantee is added. Scope selection is not evidence that the paused candidate conforms. Verify the real selected workflow under [[VER-REUSABLE-QUERY-ENGINE]] before acceptance.
:::

:::mara design DES-QUERY-FACT-COMPUTATION
:mid: 01M4GBX0TTZ0N4CWGBTR9H8AYS
:title: Supply typed facts and definition-selected derived computations
:status: draft
:kind: interface
:satisfies: REQ-EXPLICIT-QUERY-INPUTS
:satisfies: REQ-COMPOSABLE-QUERY-PROJECTIONS
:refines: DES-QUERY-INPUT-RESULT-CONTRACT

Concrete format and operation for [[REQ-EXPLICIT-QUERY-INPUTS]] and the
derived-value part of [[DES-QUERY-COMPOSITION-CONTRACT]]. It refines
[[DES-QUERY-INPUT-RESULT-CONTRACT]] for the selected adapter without changing its
semantics. Standard SPARQL still owns admitted query semantics; no second
expression evaluator is introduced.

### Definition format 5

Project-relative JSON `{format_version:5,definitions:[...]}` extends format 4.
Formats 3 and 4 are unchanged; external facts, lookups and aggregation are
rejected in them. A format-5 definition may additionally declare:

- `external_facts`: `[{input, keys, fields, missing}]`. `input` is the required
  request snapshot name; `keys` are projected tuple columns identifying one
  record that no fact, lookup or aggregation derives; `fields` maps supplied record field names to declared derived output
  columns; `missing` declares one typed value for every field output column.
- `lookups`: `[{mapping, keys, output}]`. `keys` are tuple columns; `output` is a
  declared derived column.
- `aggregate`: `{group_by, rank, precedence, retain, count, empty}`.
  `precedence` is an ordered list of typed values, highest first, resolving to
  distinct typed identities (item ID/MID aliases and signed zero are one
  identity); `count` is a
  declared integer derived column; `empty` is the declared `rank` value for a
  completely evaluated selected root with no grouped members.
- `facts_columns`: projected variable names carrying explicitly projected child
  fact provenance, excluded from tuple identity like `support_columns`. A format-4
  `dependencies[].projections.facts` alias exposes one child's fact provenance
  under that local relation column; the parent opts in by projecting it.

Derived output columns are declared in `columns` but are not part of the SELECT
tuple signature. The SELECT projects exactly `root`, the non-derived columns,
`support_columns` and `source_columns`. A caller-supplied typed value may be
bound directly as a projected tuple column: the parameter and column share a
name, and SELECT must not also bind that variable (reading it in FILTER or EXISTS
is allowed). A definition declaring
`external_facts` also declares the `external` required fact source.

### Request inputs

`query_execute` requests add optional `fact_snapshots` and `mappings`.
`fact_snapshots` entries declare `input`, an opaque `identity`, `coverage` keys
and typed `records` with `keys`/`fields`. `mappings` entries declare `mapping`,
typed `entries` (`keys`/`value`) and an optional typed `fallback`. Snapshot and
mapping names must be declared by a loaded definition and unique; unknown,
undeclared or duplicate inputs are argument errors. Full snapshot and mapping
content participates in the SHA-256 input identity, so an unchanged opaque
`identity` label never conceals changed content. Candidate and context values
are opaque typed data, never Git refs or commands.

### Evaluation

`external_facts` are overlaid in declaration order onto each returned tuple row:

- A record whose declared key columns equal the row's typed values fills the
  field output columns; provenance records the snapshot identity and record index.
- With no record for a key present in declared coverage, the declared `missing`
  values apply and provenance records covered absence.
- A missing snapshot, an unbound key column, or a required key outside declared
  coverage makes the evaluation incomplete with a reason; known bindings and
  their provenance remain.
- Malformed records, key/field type conflicts, duplicate record keys and unknown
  or duplicate inputs are invalid arguments, never silent overwrites. Supplied
  records and mapping entries are validated against every loaded declaration that
  claims them before root enumeration, so rejection never depends on which roots
  are selected or which loaded definition executes.

`lookups` then replace each declared `output` column with the entry whose typed
key equals the row's declared key values. A missing key is incomplete unless the
mapping declares a fallback; conflicting duplicate entries are invalid.

`aggregate` groups rows by the exact typed values of `group_by`, selects the
member whose `rank` value appears first in `precedence`, and retains the
`retain` and `rank` values plus the member count. A truncated, unavailable or
unranked evaluation leaves the count unbound rather than asserting an observed
member total, and the selected member's explicit
support, contributors and the union of grouped fact provenance are retained. An
unranked returned value is incomplete. A selected root whose complete evaluation
observes no grouped members yields the declared `empty` value with count zero; a
truncated or otherwise unavailable evaluation must not assert that empty outcome,
and it is not a pass or a readiness claim. Fact provenance reaches a parent only
when that parent explicitly projects the dependency's `facts` alias, like edge
support; no other hidden child provenance is reconstructed.

Types, flavours and canonical endpoints of every derived value are checked like
projected values; supplied record, coverage and mapping keys are likewise checked
against their declared columns, including item flavours. Selected roots are
evaluated independently: one root's unavailable input or reached budget marks
overall evaluation incomplete without discarding another root's known bindings.
Nothing is fetched, executed, authenticated or persisted.

### Implemented slice

`SourceEngine` and `NativeRuntime` implement format 5 and pass supplied facts and
mappings through raw queries, enabled policy validation, gates, views and
reports. CLI/MCP, daemon IPC/HTTP, Node, the shared TypeScript client and
worker/WASM expose the selected operations. The fixture is
`tests/fixtures/query-facts/snapshot.json`; Rust tests cover fact semantics,
consumers and transports; installed Node/TypeScript and Chromium tests cover the
actual adapters. [[VER-REUSABLE-QUERY-ENGINE]] still requires the integrated
matching-artifact proof and final supported-host evidence; this design records
capability, not a test pass.
:::
