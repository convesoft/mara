# Product charter

Mara is a Git-native, schema-driven engineering knowledge system. It combines
readable Markdown documents with typed items, stable identities, validation,
and traceability suitable for human review and LLM-agent development.

Its product principle is:

> Write in files, think in graph, consume in interfaces and agents.

Git-tracked source files are canonical. Every index, report, web view, and agent
context pack is a reproducible projection of a repository revision and working
tree state.

## Controlled vocabulary

:::term m_01KY7Y9R2VF93VAXEBH3EG3HXW
:id: TERM-ITEM
:title: Item
:status: accepted

An item is a schema-typed engineering object embedded in a Mara document. Every
item has a machine identity, a flavour, a body, source provenance, and any
schema-defined fields or relations.
:::

:::term m_01KY7Y9R2WJBZ6N0E2ZQX61ERH
:id: TERM-FLAVOUR
:title: Flavour
:status: accepted

A flavour is the project-defined type of an item. Flavours determine permitted
fields, relations, identity rules, and validation constraints without adding
business-object knowledge to the Mara engine.
:::

:::term m_01KY7Y9R2XJDH0X4947ATCT7MW
:id: TERM-MID
:title: Machine identity
:status: accepted
:synonym: MID

A machine identity is the mandatory, immutable, project-wide identity of an
item. Mara uses MIDs as the endpoints of every normalized internal relation.
:::

:::term m_01KY7Y9R2Y56J5YDT8YXQ5G8A1
:id: TERM-DISPLAY-ID
:title: Display ID
:status: accepted

A display ID is a mutable, human-readable project handle for an item. It is not
the item's durable identity and may be changed through a validated repository-
wide rewrite.
:::

:::term m_01KY7Y9R2ZNGEXFE8HFYXQADSK
:id: TERM-RELATION
:title: Relation
:status: accepted

A relation is a schema-declared semantic edge whose source is an item or a
permitted derived node and whose target is an item or an allowed external
object. Authored item endpoints normalize to MIDs; derived source endpoints
retain their deterministic source-span identity.
:::

:::term m_01KY7Y9R30T88X4MM61RNBYFR4
:id: TERM-INLINE-REFERENCE
:title: Inline reference
:status: accepted
:synonym: wiki link

An inline reference is a compact `[[...]]` reference written in Markdown prose
or, in later releases, a supported source-code comment. Bare references are weak
mentions; relation-qualified references are typed edges.
:::

:::term m_01KY7Y9R31VXHG10XDT1A7Q3V4
:id: TERM-DOCUMENT-MODEL
:title: Document model
:status: accepted

The document model preserves a complete Markdown document as sections,
narrative blocks, item placements, and exact source spans. Narrative content is
first-class document data without becoming an authored item automatically.
:::

:::term m_01KY7Y9R32RN20J0QQDQNQA0FY
:id: TERM-DERIVED-PROJECTION
:title: Derived projection
:status: accepted

A derived projection is a rebuildable representation of Mara source, such as a
JSON index, graph database, rendered specification, traceability matrix, or web
view. A projection is never an authoring authority.
:::

:::term m_01KY7Y9R331PN334AN6FW4ER30
:id: TERM-CONTEXT-PACK
:title: Context pack
:status: accepted

A context pack is a deterministic Markdown or JSON selection of items,
narrative, relations, source references, and provenance assembled for an
external human or software agent.
:::

## Actors

:::actor m_01KY7Y9R349G2BAH8S8SV8XRWN
:id: ACT-AUTHOR
:title: Engineering knowledge author
:status: active
:kind: human_role

An author writes and maintains Mara documents, fields, relations, and project
schemas directly in a Git working tree.
:::

:::actor m_01KY7Y9R35N591RZBEGZ3AMFH7
:id: ACT-REVIEWER
:title: Engineering reviewer
:status: active
:kind: human_role

A reviewer evaluates narrative, semantic changes, traceability, and generated
reports through ordinary Git review workflows.
:::

:::actor m_01KY7Y9R36ZRCXDGH3FP96CE9E
:id: ACT-CI
:title: Continuous integration service
:status: active
:kind: service

The CI service validates a repository revision non-interactively and publishes
stable diagnostics or derived artifacts.
:::

:::actor m_01KY7Y9R37YDMT141HT5MZ8F84
:id: ACT-AGENT
:title: Engineering agent
:status: active
:kind: agent

An engineering agent consumes bounded, traceable project context and proposes
changes while preserving the same source and validation rules as a human.
:::

:::actor m_01KY7Y9R389PPZRF1WEQ2RYQ72
:id: ACT-INTEGRATION
:title: External delivery integration
:status: active
:kind: external_system

An external delivery integration links durable Mara semantics to temporary work
coordination in systems such as Linear or GitHub without becoming authoritative
for requirements or design content.
:::

:::actor m_01KY7Y9R39J0KJQV32ZQEB6GJQ
:id: ACT-WEB-USER
:title: Web interface user
:status: active
:kind: human_role

A web interface user browses and, in a later release, edits the canonical corpus
through branch-based Git operations rather than direct database mutation.
:::

## Product goals

:::goal m_01KY7Y9R3A3JSD5SRNSGYS8RXV
:id: GOAL-GIT-CANONICAL
:title: Preserve Git as the canonical source
:status: accepted
:kind: product
:priority: must
:uses_term: TERM-DERIVED-PROJECTION

Engineering knowledge remains in auditable, branchable, mergeable source files
as the only durable authority, while every index and interface is rebuildable.
:::

:::goal m_01KY7Y9R3BXXE284ZJSHGEG9B0
:id: GOAL-SCHEMA-GENERIC
:title: Support project-defined engineering semantics
:status: accepted
:kind: capability
:priority: must
:uses_term: TERM-ITEM
:uses_term: TERM-FLAVOUR

Projects define their own flavours, fields, relations, and traceability rules
without inheriting hardcoded requirements, stories, designs, or tests.
:::

:::goal m_01KY7Y9R3CZJE18F0VHV2JNYEF
:id: GOAL-TRACEABILITY
:title: Make engineering intent traceable
:status: accepted
:kind: capability
:priority: must
:uses_term: TERM-RELATION
:uses_term: TERM-MID

Durable intent, requirements, rationale, design, implementation, verification,
and evidence form a navigable set of validated semantic edges.
:::

:::goal m_01KY7Y9R3D26FMJ4VD1M0P2GKS
:id: GOAL-READABLE-SOURCE
:title: Keep source documents readable and reviewable
:status: accepted
:kind: quality
:priority: must
:uses_term: TERM-DOCUMENT-MODEL

Mara source remains understandable in generic Markdown tooling and produces
small, reviewable Git diffs when structured content changes.
:::

:::goal m_01KY7Y9R3EM1W9G7QTEW2SVBXV
:id: GOAL-AGENT-READY
:title: Supply deterministic context to engineering agents
:status: accepted
:kind: capability
:priority: must
:uses_term: TERM-CONTEXT-PACK

External agents can consume project meaning, constraints, and provenance
directly without requiring Mara to embed a language model.
:::

:::goal m_01KY7Y9R3FH1BRMMNF16Z0P2VH
:id: GOAL-AUDITABLE
:title: Support reproducible engineering audits
:status: accepted
:kind: quality
:priority: must
:uses_term: TERM-DERIVED-PROJECTION

The same source revision, schema, and relevant working-tree state produces
deterministic semantic results and provenance suitable for review and regulated
workflows.
:::

:::goal m_01KY7Y9R3GSMSAPCQ5T2QQSN65
:id: GOAL-SCALABLE
:title: Scale from personal projects to engineering repositories
:status: accepted
:kind: quality
:priority: should

The same local workflow remains useful from dozens of artifacts through
repositories with thousands of items and large trace graphs, without requiring
a hosted service or graph database.
:::

:::goal m_01KY7Y9R3HW4VGW1WE6Z9Y38RE
:id: GOAL-BOOTSTRAP
:title: Dogfood Mara from the first implementation increment
:status: accepted
:kind: product
:priority: must
:uses_term: TERM-ITEM

Mara's own product, language, architecture, risks, and planned verification form
a valid Mara project before the initial parser exists. Validating this corpus is
the completion outcome for the first implementation increment.
:::

## Scope boundary

The v0.1 product is a deterministic local Rust CLI and reusable library. It
initializes projects, loads and validates schemas, parses Mara Markdown, resolves
the graph, reports diagnostics, supports inspection and safe display-ID
renaming, and emits a JSON projection.

Graph databases, semantic diff, rendered specifications, context-pack
generation, source-code scanning, delivery-provider synchronization, and the
web interface are explicitly post-v0.1 capabilities. Their architectural
boundaries are preserved now so the bootstrap model does not block them later.

Linear or similar services manage execution priority, ownership, and delivery
status. Git hosting manages code review, CI, and merge evidence. Mara owns
durable engineering meaning and traceability.
