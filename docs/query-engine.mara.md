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
contracts and the user's contract-authoring answers. Backend choice, persisted
definition language/profile, format versions, exact public operations/encodings
and numeric evaluation budgets remain pending the proof; illustrative YAML or
proposed commands do not select them. Do not build a custom query DSL or
framework without explicit approval.

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

Definition resolution, explicit roots/parameters, typed bindings, absence, provenance and input/result completeness follow [[DES-QUERY-INPUT-RESULT-CONTRACT]].
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

A bounded backend proof must execute one query definition both directly and
through the SHACL constraint bridge. Reuse applicable evaluator literal,
qualification and logic behavior, without letting that reuse override the
selected semantic contracts. Backend choice, persisted definition syntax/profile,
format versions, concrete public operations/encodings, numeric evaluation budgets
and normalization/ordering encodings remain pending that proof. The bounded proof subset and subsequent integrated
fixture/host checks are distinguished in [[VER-REUSABLE-QUERY-ENGINE]].
A proof conflict requires a user decision, not silent reduction or a custom
DSL/framework.

These draft designs specify behavior to prove. They do not certify the backend,
SHACL bridge, new operations or host parity. Engineering-template refinement and
self-hosted schema/policy changes remain outside this engine initiative.
:::


:::mara design DES-QUERY-INPUT-RESULT-CONTRACT
:mid: 01M4BZ3CY8E7GJ9317QWDZP7ZF
:title: Bind reusable query facts to explicit typed inputs
:status: draft
:kind: interface
:satisfies: REQ-NAMED-QUERY-FACTS
:satisfies: REQ-EXPLICIT-QUERY-INPUTS

Semantic contract for the selected 0.4 query engine, not a serialized API or
implemented capability. The contract-authoring user clarification selected
strict identity, selection and typed-input behavior.

### Definitions and schema binding

A caller/host loads an explicit project/template definition set for one project
and selects an exact definition identity. Identities are unique across that set;
duplicates, unresolved references and recursive dependencies are definition
errors. No load-order precedence, implicit template override, fuzzy lookup,
network import or cross-project resolution applies. Identity spelling, namespace
encoding and persisted definition syntax remain for the backend proof.
References bind to the selected definition content, not merely its name.

Bind flavour, field, relation and inverse-alias references against the selected
schema before execution. Unknown vocabulary, incompatible endpoint/value types
or incompatible composition signatures are source-linked definition errors.
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

Each returned fact/derived value retains its root, contributing typed bindings,
definition/derivation references and source support: document locations,
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
success. Representation, version numbers and exact public errors remain pending
the bounded backend proof.
:::

:::mara design DES-QUERY-COMPOSITION-CONTRACT
:mid: 01M4BZ3SV8DTX6ZV7XVY8270R8
:title: Compose distinct bindings and finite graph reachability
:status: draft
:kind: behavior
:satisfies: REQ-COMPOSABLE-QUERY-PROJECTIONS

Semantic composition consumes [[DES-QUERY-INPUT-RESULT-CONTRACT]]; no new
definition language is selected here.

### Composition and equality

Named-query dependencies form an acyclic graph. Resolve/type-check the reachable
definitions before execution; direct/indirect recursion is invalid. Compose
results through joins on explicit compatible typed keys, unions of compatible
projections, filters and derived projections. Inner joins retain matching
bindings; explicit left joins retain unmatched left bindings as absence.
Neither removes the selected-root records when its match set is empty.
A composed child receives explicitly bound roots/context/parameters; ambient
project-wide roots and implicit workload expansion are not allowed.

Results use distinct typed binding tuples by default. Repeated authored
assertions, alias spellings or multiple supporting paths do not multiply an
identical binding. Combine its supporting provenance. Projection may merge
bindings that become identical after columns are removed, retaining their
combined support. Union uses the same distinctness. Joins match typed identity,
not titles or formatted cells; support from both operands remains attached.

A definition specifies the counted binding/column and qualification.
Ordinary counts count distinct tuples/endpoints, not authored assertions or
paths. Explicit assertion-occurrence counting counts distinct authored occurrence
identities, even if the same occurrence is encountered by two paths; pagination
never changes counts. Unbounded walk/path counting is not part of this contract.

### Traversal

Definitions choose relation kinds, direction, qualification and direct versus
transitive expansion. Ancestry expands only the declared upward relation;
ancestor context and its related needs/goals/risk support do not become workload,
and reverse descent into siblings is not implicit. External addresses are
terminal unless explicitly joined to supplied fact records; URLs are not graphs
to dereference.

Transitive reachability returns each qualifying reachable endpoint once.
Ancestry excludes its starting root, including when a nonzero-length cycle
returns to it; include-self must be explicit. Track visited identities to
terminate finite graph cycles while retaining finite supporting provenance.
Cycle presence does not itself invalidate facts: project schema acyclicity
remains [[REQ-RELATION-CYCLE-POLICY]]'s independent validation obligation.
A constrained cycle can therefore be inspectable facts and a documentation
violation, not a query-engine permission to relax the schema.

If an evaluation resource limit stops required expansion/composition, mark it
incomplete/unavailable, retain any known support with the limit reason, and
never infer a complete negative, exact count or policy pass from truncation.
A definition's intentional bounded-hop question is different: fully evaluating
that declared domain can be complete. Numeric budgets, supported bound controls,
cursor encoding and concrete traversal implementation remain for the proof.

### Derived values, logic and mappings

Equality/text comparison is exact and type-aware by default. Definitions may
explicitly apply deterministic Unicode NFC normalization and full default
(nonlocale) case folding for title/alias collisions; preserve original text and
support. Do not add fuzzy matching or locale-dependent behavior. The proof must
pin the concrete normalization behavior consistently across hosts.

Existence and any on a complete empty set are false; all and none are true;
a distinct count is zero. These are query facts, not readiness judgments.
Missing required input cannot be converted into any of those empty-set values.
Unknown operands remain explicit; a predicate may be known when complete
available support logically settles it, but unavailable required dependencies
remain visible and completeness is not upgraded. Gates follow
[[DES-QUERY-CONSUMER-CONTRACT]], including explicit empty-set interpretation.

Definitions select aggregate/grouping keys, qualification and result precedence;
there is no built-in ordering of pass/fail/inconclusive/error/pending, lifecycle
rank or risk formula. An aggregate declares its empty case and retains members
and support. Conflicting or underspecified precedence is a definition error,
not load-order selection.

Lookup mappings are typed caller/project policy data bound into input identity.
Validate key/value types and reject conflicting duplicate keys. A missing key
is unavailable unless an explicit fallback is declared; it never silently maps
to zero, low risk, pass or no match. Fixture-defined likelihood/consequence
scores, thresholds and reassessment use these mechanisms. Implementation links
do not automatically lower a risk or select a new assessment.
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

Real native/browser and installed-package verification belongs to
[[VER-REUSABLE-QUERY-ENGINE]] and the selected final supported-host gates.
This contract does not claim those paths ran. Exact operations/public formats,
SHACL bridge and backend feasibility remain pending the bounded proof.
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
   an unresolved/recursive query reference, an unknown schema name and a
   type-incompatible relation/join are definition errors. Missing/wrong-type/
   undeclared parameters and unknown explicit item IDs are argument errors.
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
   count is one and all contributing source support remains inspectable.
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
contracts; unsupported semantics require a user decision. The proof then selects
backend/profile, persisted syntax, format versions, exact public operations and
evaluation budgets. No custom DSL/framework is authorized by this verification.

The complete fixture catalog and selected host parity are subsequent integrated
engine checks, not required execution for the contract-only deliverable. Use the
existing supported-host gates, with Apple Silicon execution at the final host
gate. Record actual tested revisions, artifacts, environments, commands, results
and limitations. A fixture demonstration is not adoption of the engineering
template, authentication of reported external evidence or proof of external
execution.
:::
