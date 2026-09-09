# Guided authoring

Planning scope for 0.2.0: a solo developer can start useful engineering
documentation, choose its vocabulary, and give an agent access to document
context as well as items. The scenarios and requirements below describe future
outcomes; they do not claim implementation or settle the open designs below.

Prioritize bundled templates and flavour guidance together, then useful
engineering relations and document access. Diagnostic codes and severity are
deferred beyond 0.2 until a concrete consumer workflow demonstrates a need.

This scope uses one Mara project and schema, including documents inside package
directories. It does not require workspace aggregation, template inheritance,
remote template packs, configuration composition, or a mandatory traceability
process. The existing 0.1 stabilization sequence remains separate; this plan
does not add these features to alpha.3, beta, or release-candidate acceptance.

## Intended workflows

:::mara scenario SCN-START-ENGINEERING-KNOWLEDGE
:mid: 01M1XSJZVEZKVRBMCWK9GFQ3XC
:title: Start a project with an engineering vocabulary

An author selects an engineering template, initializes a project, and creates
requirements, designs, and verification knowledge using the supplied vocabulary.
The resulting schema is ordinary project-owned data. No Mara product documents,
existing item identities, or placeholder trace chains are copied into the project.
:::

:::mara scenario SCN-CHOOSE-KNOWLEDGE-FLAVOUR
:mid: 01M1XSJZVN9VWH6TP58DX6WH54
:title: Choose a flavour from project guidance

An author or agent inspects the selected project's schema guidance before
creating an item. The guidance explains each flavour's purpose, when to use or
avoid it, and how it differs from confusable flavours. The author can select a
flavour without relying on knowledge of Mara's own repository taxonomy.
:::

:::mara scenario SCN-READ-DOCUMENT-CONTEXT
:mid: 01M1XSJZVV5V98210YBNJ3XYY4
:title: Retrieve narrative context through Mara

An agent using Mara's interface discovers relevant canonical document content
outside item blocks, including a document with no items, then reads the needed
source context in bounded portions. Results identify the source location without
inventing an item identity or requiring the author to turn narrative into items.
The same workflow is available through CLI and MCP.
:::

## Intended requirements

:::mara requirement REQ-ENGINEERING-TEMPLATE
:mid: 01M1XSKPP0SRTDXBZE3J2PVDJ8
:title: Initialize from a bundled engineering template
:derives_from: SCN-START-ENGINEERING-KNOWLEDGE

Offer an optional `engineering` template based on the reusable vocabulary in
[the self-hosting taxonomy](taxonomy.mara.md). Keep `minimal` as the default and
retain `empty`. Initialization preserves [[REQ-PROJECT-INITIALIZATION]] and
generates only `.mara/project.toml` and `.mara/schema.yaml`, with no starter
Markdown or separate guidance document. The schema is editable project-owned
data; templates do not copy Mara's product items or MIDs.

Maintain template content in source files bundled into the executable, without
requiring a runtime template directory. Changing a bundled template must not
silently rewrite schemas in projects already initialized from it.
:::

:::mara requirement REQ-FLAVOUR-AUTHORING-GUIDANCE
:mid: 01M1XSKPP7HRP5JKTDX05EFE04
:title: Require project-defined flavour selection guidance
:derives_from: SCN-CHOOSE-KNOWLEDGE-FLAVOUR

In 0.2.0, every declared flavour must provide guidance covering its purpose,
when to use it, when to avoid it, and distinctions from other flavours. Keep the
existing `description` for purpose and add `use_when`, `avoid_when`, and
`distinguish_from`. Missing required guidance is a schema validation error,
including for existing project schemas; it is not optional legacy behavior.
An empty schema has no flavours that require guidance.

CLI and MCP schema inspection must expose the same project-defined guidance
under [[REQ-SCHEMA-DISCOVERY]] and [[REQ-SURFACE-PARITY]]. Bundled templates must
supply complete guidance for their declared flavours. Guidance must not
introduce hardcoded business flavours in the engine.

The value types, nesting, empty-value rules, and schema-format transition remain
open design questions. This requirement is a breaking change for 0.2.0; it does
not change 0.1 schema validation.
:::

:::mara requirement REQ-ENGINEERING-TRACEABILITY
:mid: 01M1XSKPPD0BFBNPGTZQKG6P0T
:title: Connect engineering knowledge with meaningful typed relations
:derives_from: SCN-START-ENGINEERING-KNOWLEDGE

The engineering template must provide relations for connecting verification to
its targets, evidence to verification, implementation artifacts to requirements
or designs, and risks to affected knowledge and mitigation. Define the relation
meanings and allowed endpoints in the project vocabulary before adding them.

Authors add only links that carry useful meaning. The template must not require
placeholder items, a complete trace chain, or a particular lifecycle. Existing
relation mutation and retrieval operations remain the mechanism for these links;
this requirement does not introduce a graph-rule engine.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-DISCOVERY
:mid: 01M1XSKPPMJAFMZK0WD8243M4P
:title: Discover canonical context outside item blocks
:derives_from: SCN-READ-DOCUMENT-CONTEXT

CLI and MCP must let callers discover matching content outside item blocks in
the selected project's canonical documents, including narrative-only documents.
Discovery must return bounded results with source locations and an explicit way
to continue when more results remain. It must respect project content discovery
and distinguish document context from identified items.

The result unit, matching and ranking, path filters, and whether discovery covers
whole documents or narrative-only passages remain open. Reuse the investigation
in [retrieval](retrieval.mara.md#narrative-retrieval-investigation); do not assign
synthetic item IDs or typed relations to narrative.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-READ
:mid: 01M1XSKPPTZCGKHDSYW0SKMB6B
:title: Read document context completely through bounded portions
:derives_from: SCN-READ-DOCUMENT-CONTEXT

CLI and MCP must let callers read selected canonical document context in bounded
consecutive portions with source locations, explicit continuation, and an
explicit completion signal. Following continuation must recover the selected
content without silent gaps, including oversized Markdown and narrative-only
documents. Continuation must not silently combine different source revisions.

The command surface, read unit, limits, and source-change behavior must be
settled before implementation. The current alpha.3 file-access decision
[[ADR-ALPHA-NARRATIVE-FILE-ACCESS]] remains its own release boundary.
:::

## Decisions needed before implementation planning

| Area | Open decision |
|---|---|
| Flavour guidance | The names `description`, `use_when`, `avoid_when`, and `distinguish_from` are settled. Choose value types, nesting, empty-value rules, and the schema-format transition. Reconcile the existing taxonomy with schema-owned guidance so each definition has one authority. |
| Engineering relations | Confirm names, direction, and endpoint flavours for verification, evidence, implementation, and risk links. Candidate names are `verifies`, `validates`, `evidences`, `implements`, `affects`, and `mitigates`; this document does not add them to the schema. |
| Document access | Discuss after engineering relations. Choose the public discovery/read surface and resolve the result-unit, ranking, filtering, continuation, and compatibility questions in the retrieval investigation. |

After settling each area's product choices, record its interface or persisted
contract as a design and consequential rationale as a decision. Delivery tickets
reference those items and the requirements above; verification belongs with each
implemented outcome. The full 0.2 ticket breakdown follows this scope review.

## Decisions and migration

:::mara requirement REQ-FLAVOUR-GUIDANCE-MIGRATION
:mid: 01M22ZQ349GZHVT2F2R1MRX90G
:title: Document migration to mandatory flavour guidance
:derives_from: REQ-FLAVOUR-AUTHORING-GUIDANCE

The 0.2.0 release must identify mandatory flavour guidance as a breaking schema
change and link a canonical migration guide from its release notes. Version the
incompatible persisted schema contract independently of the application version.

The guide must provide the supported schema-format transition and before/after
YAML examples once the persisted guidance shape is settled. It must explain how
to preserve custom flavours, fields, relations, and item identities while adding
guidance to every declared flavour, then verify schema and project validation
through CLI and MCP. Do not instruct users to reinitialize an existing project
or replace a customized schema with a bundled template.

Verification must demonstrate that a schema missing required guidance is
rejected by 0.2 and that following the guide makes it valid without unrelated
corpus changes. No finished YAML migration recipe is specified until the new
schema shape is accepted.
:::

:::mara decision ADR-MANDATORY-FLAVOUR-GUIDANCE
:mid: 01M22ZQ34GZX9EKMPR81DH87P3
:title: Require complete flavour guidance starting with 0.2
:justifies: REQ-FLAVOUR-AUTHORING-GUIDANCE
:justifies: REQ-FLAVOUR-GUIDANCE-MIGRATION

Apply mandatory flavour guidance to existing project schemas as well as bundled
templates in 0.2.0. Accept a documented breaking change and explicit migration
instead of retaining an optional-guidance exception for older schemas.

The project accepts the migration cost at this early adoption stage so authors
and agents can rely on guidance for every declared flavour. The earlier alpha
schema contract does not justify making that guarantee permanently optional.
The implementation and migration guide belong to 0.2; the 0.1 release retains
its existing schema behavior.
:::

:::mara decision ADR-SCHEMA-ONLY-TEMPLATES
:mid: 01M230NZB3DX52SQDS91XXF06A
:title: Generate configuration and schema without starter documents
:justifies: REQ-ENGINEERING-TEMPLATE

Bundled templates generate project configuration and schema only. Flavour
guidance belongs in the schema and is exposed through Mara's schema inspection.
Do not generate starter Markdown or a second guidance document.

This keeps initialization small, gives project authors ownership of document
structure, and avoids maintaining duplicate guidance in schema and Markdown.
:::
