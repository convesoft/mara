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
engine. The definition language/profile, result formats, operation names and
cycle/limit semantics require explicit design backed by that proof; they are
not settled by illustrative YAML or proposed command names. Do not build a
custom query DSL or framework without explicit approval.

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
:::

:::mara requirement REQ-COMPOSABLE-QUERY-PROJECTIONS
:mid: 01M4BP8KCRXSB29201PHSAYZA6
:title: Compose graph facts and derived projections without domain code
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Allow named queries to compose through joins, unions, explicit transitive traversal and derived projections. Support counts, existence and all/any/none conditions, deterministic text normalization/comparison, result aggregation with definition-selected precedence, and caller/project-supplied lookup mappings. Preserve supporting bindings instead of reducing every derived result to a boolean.

Make traversal direction, duplicate/counting behavior, cycle termination and evaluation limits explicit. A transitive ancestry query expands context without silently expanding caller-selected workload or walking back down into siblings. No matches, a missing execution result, and an unavailable computation remain distinguishable. Engineering-specific formulas, lifecycle selection, matrices and thresholds belong in definitions/inputs, not Rust branches.
:::

:::mara requirement REQ-EXPLICIT-QUERY-INPUTS
:mid: 01M4BP904M6HT15VPWQZ056VQD
:title: Consume host-supplied facts and context without owning execution
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Evaluate queries over durable graph knowledge and explicitly supplied external fact snapshots, parameters and organization policy. Support caller-selected workload, candidate identities and opaque validation contexts without interpreting them as commands or fetching data implicitly. Retain provenance and input identity needed to explain the result and invalidate stale continuation or reused outcomes.

An absent result in a usable supplied snapshot differs from unavailable required input. Query execution does not run tests, authenticate reported evidence, persist transient results as documentation, or acquire deployment/provider credentials. Native code discovery remains host-owned; browser evaluation may consume supplied facts without introducing a browser code indexer.
:::

:::mara requirement REQ-QUERY-POLICY-CONSUMERS
:mid: 01M4BP9D1A4MX4E078ZERNR202
:title: Separate documentation validity from contextual gate outcomes
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Documentation rules consume reusable query facts and impose enabled invariants contributing to project/item validation. Contextual gates consume queries plus explicit scope/context and report whether their declared conditions hold without changing documentation validity. Neither consumer duplicates the query's traversal definition.

Preserve applicability, severity where relevant, supporting bindings and source-linked explanations. Distinguish passed, failed, not applicable and unavailable outcomes; missing prerequisites cannot become a policy pass. Source conformance and schema graph constraints remain validation responsibilities independent of query policy. Gates report conditions, not authorization or automatic action execution. Agent semantic judgment remains outside deterministic enforcement.
:::

:::mara requirement REQ-QUERY-VIEWS-AND-REPORTS
:mid: 01M4BP9SY141XDMXE827KVFK4W
:title: Render factual views separately from policy explanations
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Provide factual tables/relationship matrices over query results and explanation reports over documentation-rule/gate outcomes. Views preserve bindings, relevant source links, input identity and completeness without selecting different graph facts or inventing obligations. Compose queries/projections before rendering; presentation is not a separate semantic mechanism.

A root with no related matches can appear in a factual view without being marked failed. An evaluation report identifies the obligation, outcome and qualifying or missing support. Reuse useful existing trace explanation behavior without treating the current trace-matrix operation as the new factual query API.
:::

:::mara requirement REQ-QUERY-ENGINE-SURFACES
:mid: 01M4BPA6PXXQA31XTGSHVZVXFH
:title: Expose shared query and policy semantics across supported consumers
:status: draft
:kind: functional
:derives_from: SCN-REUSE-QUERY-FACTS

Expose raw named-query execution, documentation-rule validation, contextual gates and factual/evaluation rendering through the shared Rust engine and the selected 0.4 native and browser application paths: embedded/daemon Rust, CLI/MCP, Node binding, shared TypeScript client, daemon HTTP and worker/WASM. Use one semantic implementation, with typed input/result contracts and consistent errors and continuation.

Hosts load or supply the required sources/facts; adapters do not independently traverse the graph or reinterpret policy. Honor existing host capability boundaries and report unavailable required inputs explicitly. Verify real native and browser execution rather than treating a compile or mock adapter as semantic parity.
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

Within mara-engine, share canonical graph preparation, typed values, normalized relationship identities and source mappings. Named query evaluation produces facts/bindings with provenance and completeness. Documentation rules and contextual gates consume those results; renderers consume facts or evaluations. Native hosts and browser workspaces supply inputs through the execution boundary in [[DES-EXECUTION-BOUNDARIES]].

Keep schema vocabulary and engineering semantics in project/template definitions. External fact acquisition belongs to the host; no provider framework is required. Do not reconstruct complete query results from SHACL evaluation caches, whose observations reflect only evaluated constraints.

A bounded backend proof must establish query execution and the SHACL constraint bridge using the same query definition before the definition language/profile and public formats are selected. Reuse existing evaluator semantics where applicable rather than independently reimplementing literals, qualification and logic. Record the selected backend, formats, operation names, traversal limits and result contracts here or in focused linked designs after the proof; unsupported approaches require a decision rather than silent feature reduction.
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

Use disposable fixture projects with fixture-owned vocabulary, definitions, source knowledge and external fact snapshots. Keep synthetic assertions out of the self-hosted corpus. Exercise the actual engine and application paths; supplied runtime results are test inputs, not proof that the engine ran external tests.

Demonstrate these capability witnesses without ontology-specific Rust code:

- One query returns draft/accepted/retired links and empty roots; raw output, a factual view, a documentation rule and a contextual gate reuse it. Show valid-but-incomplete knowledge versus an invariant violation and a candidate gate failure.
- Basis alternatives, inverse verification links, status-bearing lifecycle bindings, deterministic case-normalized title/alias collisions, decomposition and supersession projections.
- Direct implementation or implementation through an accepted satisfying design, retaining supporting bindings; no design-mandatory rule is built into the engine.
- Explicit workload with transitive ancestors, related needs/goals and explaining risk bindings, excluding sibling workload. Exercise duplicate paths and the selected cycle/limit contract.
- Candidate-specific verification results with every accepted verification required by the fixture gate; missing definitions/results must not become vacuous readiness success. Context-specific validation aggregation distinguishes pass/fail/inconclusive/error and pending according to fixture definitions, including explicit required-validation selection.
- Replaceable likelihood/consequence mappings and risk thresholds supplied as policy data, with explicit reassessment rather than automatically lowering risk after implementation.
- Definition/argument errors, unavailable required inputs, stable continuation and invalidation after definition/source/context/fact changes; views must not alter factual or policy results.

Run real CLI/MCP and native embedded/daemon workflows plus installed Node/TypeScript and Chromium HTTP/worker-WASM paths for the shared operations. Check definition/backend portability before broad implementation. Use the existing supported-host gates, with Apple Silicon execution at the final host gate. Record actual tested revisions, artifacts, environment, commands, results and limitations. A fixture catalog demonstration is not adoption of that engineering template or evidence of external execution.
:::
