# Agent context and future interfaces

Mara v0.1 stops at a deterministic local semantic model and JSON projection.
The capabilities in this document reserve how later agents, graph stores,
delivery integrations, and web clients consume that model without becoming an
alternative source of truth. They are deliberately proposed rather than part of
the v0.1 delivery baseline.

## Deterministic agent context

:::req m_01KY7YA2DX4DMDTMZQ9J56Q7BK
:id: REQ-AGENT-NO-EMBEDDED-LLM
:title: Mara semantic processing shall not require an embedded language model
:status: proposed
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AGENT-READY

Parsing, normalization, reference resolution, validation, traversal, diffing,
and context selection shall remain deterministic program behaviour. A future
LLM may consume Mara output or propose source edits through an explicit adapter,
but model inference shall not determine the canonical project graph.
:::

:::req m_01KY7YA2DYJ03SWP33Y4M48W5W
:id: REQ-CONTEXT-PROFILES
:title: Projects shall define named context traversal profiles
:status: proposed
:level: system
:kind: functional
:priority: must
:derives_from: STORY-AGENT-CONTEXT
:uses_term: TERM-CONTEXT-PACK

A schema shall be able to define named context profiles by focus flavours,
allowed incoming and outgoing relations, traversal depth, included fields,
narrative inclusion, and deterministic size limits. The engine shall not embed
a universal implementation, review, or testing process in those profiles.
:::

:::req m_01KY7YA2DZ8DBBNS8ANEH9MQG3
:id: REQ-CONTEXT-OUTPUT
:title: Context packs shall have deterministic Markdown and JSON forms
:status: proposed
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AGENT-CONTEXT
:uses_term: TERM-CONTEXT-PACK

`mara context <ref> --profile <name>` shall render a human-readable Markdown
pack and a complete JSON pack from the same selected subgraph. Equivalent input,
schema, Git state, profile, and command options shall produce byte-equivalent
output.
:::

:::req m_01KY7YA2E0N5BQDY2JTHEMKQCX
:id: REQ-CONTEXT-PROVENANCE
:title: Every context entry shall preserve selection provenance
:status: proposed
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-AUDITABLE
:uses_term: TERM-CONTEXT-PACK

Each included item or narrative fragment shall identify its MID when applicable,
current display ID, project-relative source span, relation path or selection rule,
schema version, and available Git revision and dirty-state provenance. Truncation
shall be explicit and shall never silently omit a required profile component.
:::

## Change and view projections

:::req m_01KY7YA2E112KVBJK76JZA3HRM
:id: REQ-SEMANTIC-DIFF
:title: Mara shall compare project revisions by semantic identity
:status: proposed
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-TRACEABILITY

A semantic diff shall compare two Git revisions or worktree states by MID and
report added and removed items, display-ID renames, field and body changes,
relation changes, document relocation, and directly impacted neighbours. File
movement or formatting-only edits shall not appear as semantic replacement.
:::

:::req m_01KY7YA2E2ZSBM4E6FJ255K2ND
:id: REQ-SCHEMA-DEFINED-VIEWS
:title: Projects shall define named document and matrix views
:status: proposed
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-SCHEMA-GENERIC

A schema shall be able to define named specification, catalogue, and traceability
matrix views from flavour, field, relation, ordering, and narrative-selection
rules. PRD, SRS, V-model matrix, and similar methodology names shall be project
configuration rather than engine business objects.
:::

## Source, graph, delivery, and web adapters

:::req m_01KY7YA2E36WXE3QBXEKN4HA7S
:id: REQ-SOURCE-CODE-REFERENCES
:title: Source scanners shall create language-aware derived trace nodes
:status: proposed
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-TRACEABILITY

Configured source scanners shall recognize `[[relation:REF]]` only inside
comments or other language-defined annotation locations. A scanner shall attach
the resolved MID edge to a project-relative file and exact span, and may attach
it to a language symbol when a reliable parser supplies that identity. Raw
substring scanning across executable code shall not create authoritative edges.
:::

:::req m_01KY7YA2E4RF51RVTVHZZZCNA6
:id: REQ-GRAPH-PROJECTION-BACKENDS
:title: Graph databases shall remain replaceable derived projections
:status: proposed
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-GIT-CANONICAL
:uses_term: TERM-DERIVED-PROJECTION

Future graph backends may accelerate traversal, search, visualization, and
context assembly, but shall be rebuildable from a selected repository state.
No graph record shall become an independently editable semantic authority.
:::

:::req m_01KY7YA2E5JC5B3B14C6QSS03Y
:id: REQ-WEB-GIT-AUTHORING
:title: The web interface shall author through isolated Git worktrees
:status: proposed
:level: system
:kind: safety
:priority: should
:derives_from: STORY-WEB-AUTHORING

A future web service shall first provide read-only project navigation. Mutating
sessions shall operate on an isolated branch and worktree, apply source-span-
checked transactions, validate before presenting changes, and leave commit and
pull-request policy explicit. The service shall never edit a graph projection as
the source of a Markdown change.
:::

:::req m_01KY7YA2E6X1CCBT9ZK9JXW2ZP
:id: REQ-EXTERNAL-ADAPTER-BOUNDARY
:title: Delivery adapters shall not own Mara semantic content
:status: proposed
:level: system
:kind: constraint
:priority: must
:derives_from: STORY-LINK-DELIVERY

Adapters for Linear, GitHub, or another execution system may resolve configured
external URIs, create or update delivery work, and attach delivery provenance.
They shall not overwrite item bodies, semantic fields, relations, or lifecycle
states from external operational fields. Mara owns durable meaning; execution
systems own assignment, priority, schedule, and delivery workflow.
:::

## Proposed design and risk

:::design m_01KY7YA2E7R2MGEEB037HKX6JT
:id: DES-CONTEXT-COMPILER
:title: Profile-driven provenance-preserving context compiler
:status: proposed
:kind: component
:satisfies: REQ-AGENT-NO-EMBEDDED-LLM
:satisfies: REQ-CONTEXT-PROFILES
:satisfies: REQ-CONTEXT-OUTPUT
:satisfies: REQ-CONTEXT-PROVENANCE
:satisfies: REQ-SCHEMA-DEFINED-VIEWS

The context compiler shall select from the normalized project model using only
schema and command input, then render through output adapters. Selection records
shall remain available to explain why every fragment was included.
:::

:::design m_01KY7YA2E87DZ54QNJ6R1EXW93
:id: DES-SEMANTIC-ADAPTER-LAYER
:title: Read model and integration adapter layer
:status: proposed
:kind: architecture
:satisfies: REQ-SEMANTIC-DIFF
:satisfies: REQ-SOURCE-CODE-REFERENCES
:satisfies: REQ-GRAPH-PROJECTION-BACKENDS
:satisfies: REQ-WEB-GIT-AUTHORING
:satisfies: REQ-EXTERNAL-ADAPTER-BOUNDARY

Future interfaces shall consume versioned engine services and derived read
models. Source scanners contribute derived nodes, projection adapters persist
read models, and write adapters can request the same transaction service used
by the CLI without bypassing source validation.
:::

:::decision m_01KY7YA2E9B35TRYZ1SE9K4GHR
:id: ADR-0009
:title: Keep inference and external workflow outside the semantic kernel
:status: accepted
:kind: llm_workflow
:justifies: REQ-AGENT-NO-EMBEDDED-LLM
:justifies: REQ-EXTERNAL-ADAPTER-BOUNDARY
:justifies: DES-CONTEXT-COMPILER
:justifies: DES-SEMANTIC-ADAPTER-LAYER

Mara must be reproducible in CI and usable by different agents and delivery
systems. The engine therefore compiles trustworthy context and accepts explicit
source changes, while probabilistic reasoning and operational workflow remain
replaceable clients.
:::

:::risk m_01KY7YA2EA5HKVWBFJ6T9WE3YY
:id: RISK-CONTEXT-OMISSION
:title: Bounded context may hide a governing constraint
:status: open
:severity: high
:likelihood: medium
:affects: REQ-CONTEXT-PROFILES
:affects: REQ-CONTEXT-PROVENANCE
:affects: DES-CONTEXT-COMPILER

A compact agent pack can appear complete while excluding a relevant upstream
decision, risk, or requirement. Named profiles therefore need explicit mandatory
relations, visible truncation, deterministic limits, and provenance suitable for
independent review.
:::

## Planned future verification

:::test m_01KY7YA2EBBYFT415FNW303WQ4
:id: TEST-CONTEXT-COMPILER
:title: Deterministic context and provenance test
:status: draft
:kind: verification
:method: automated
:level: system
:verifies: REQ-AGENT-NO-EMBEDDED-LLM
:verifies: REQ-CONTEXT-PROFILES
:verifies: REQ-CONTEXT-OUTPUT
:verifies: REQ-CONTEXT-PROVENANCE
:verifies: REQ-SCHEMA-DEFINED-VIEWS

Golden projects shall exercise profile traversal, narrative selection, cycles,
mandatory context, deterministic truncation, Markdown and JSON equivalence, and
complete per-entry provenance without invoking a model or network service.
:::

:::test m_01KY7YA2ECCFHZSP8ZY6NSBQX0
:id: TEST-FUTURE-ADAPTER-BOUNDARIES
:title: Future adapter authority and projection test
:status: draft
:kind: verification
:method: automated
:level: system
:verifies: REQ-SEMANTIC-DIFF
:verifies: REQ-SOURCE-CODE-REFERENCES
:verifies: REQ-GRAPH-PROJECTION-BACKENDS
:verifies: REQ-WEB-GIT-AUTHORING
:verifies: REQ-EXTERNAL-ADAPTER-BOUNDARY

Tests shall compare formatting-only and semantic revision changes, reject source
markers outside valid comments, rebuild graph projections, exercise isolated
web worktrees, and prove that external operational updates cannot mutate Mara
semantic source.
:::
