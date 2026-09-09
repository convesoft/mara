# Guided authoring

Planning scope for 0.2.0: a solo developer can start useful engineering
documentation, choose its vocabulary, and give an agent access to document
context as well as items. The scenarios and requirements below describe future
outcomes; they do not claim implementation or settle the open designs below.

Prioritize bundled templates and flavour guidance together, then useful
engineering relations and unified discovery. Diagnostic codes and severity are
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
:title: Discover narrative and follow its connections through Mara

An actor searches the selected project's canonical documentation through Mara
and finds either a structured item or an ordinary narrative passage, including
content in documents without items. Starting from a passage's explicit mention,
the actor inspects the referenced requirement and then its direct connections
to designs or decisions, choosing each subsequent step.

Search and neighbour results expose source locations and connection meaning.
The actor reads the needed source context using existing file tools with access
to the same repository. Authors need not turn narrative into items. Discovery
and direct navigation have equivalent CLI and MCP behavior; richer graph
analysis and code-symbol extraction are later work.
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
when to use it, when to avoid it, and distinctions from other flavours. Missing
required guidance is a schema validation error, including for existing project
schemas. An empty schema has no flavours that require guidance.

The persisted keys, value types, and content-validation rules follow
[[DES-FLAVOUR-AUTHORING-GUIDANCE]]. CLI and MCP schema inspection must expose the
same project-defined guidance under [[REQ-SCHEMA-DISCOVERY]] and
[[REQ-SURFACE-PARITY]]. Bundled templates must supply complete guidance for
their declared flavours without hardcoded business flavours in the engine.

This is a breaking change for 0.2.0; it does not change 0.1 schema validation.
Migration follows [[REQ-FLAVOUR-GUIDANCE-MIGRATION]].
:::

:::mara requirement REQ-ENGINEERING-TRACEABILITY
:mid: 01M1XSKPPD0BFBNPGTZQKG6P0T
:title: Connect engineering knowledge with meaningful typed relations
:derives_from: SCN-START-ENGINEERING-KNOWLEDGE

The engineering template must provide relations for connecting verification to
its targets, evidence to verification, implementation artifacts to requirements
or designs, and risks to affected knowledge and mitigation. Names, meanings,
directions, and allowed endpoints follow [[DES-ENGINEERING-RELATION-VOCABULARY]].

Authors add only links that carry useful meaning. The template must not require
placeholder items, a complete trace chain, or a particular lifecycle. Existing
relation mutation and retrieval operations remain the mechanism for these links;
this requirement does not introduce a graph-rule engine.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-DISCOVERY
:mid: 01M1XSKPPMJAFMZK0WD8243M4P
:title: Discover canonical context outside item blocks
:derives_from: SCN-READ-DOCUMENT-CONTEXT

CLI and MCP must discover matching items and narrative passages through one
bounded search surface in the selected project's canonical documents, including
narrative-only documents. Move the CLI entry point to `mara search`. Return
explicit result kinds, source locations, and continuation when more matches
remain. Do not add a separate document-search operation.

Passages are first-class discovery nodes without requiring authored item IDs,
MIDs, or flavours. Their explicit connections follow
[[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]]. Result boundaries, filters, source reading,
and remaining interface details follow [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].

Expose structural membership under [[DES-DOCUMENT-STRUCTURE]] so an actor can
identify the containing section or document and navigate from it.

Verify mixed item/passage results, narrative-only documents, path and item-only
filters, bounded continuation, and absence of duplicated item-body hits through
CLI and MCP.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-READ
:mid: 01M1XSKPPTZCGKHDSYW0SKMB6B
:title: Locate discovered context for existing source-reading tools
:derives_from: SCN-READ-DOCUMENT-CONTEXT

Discovery and direct-neighbour results must identify the source document and
exact location needed to read the selected item or narrative passage using the
actor's existing file tools. The workflow assumes access to the same project
sources. Excerpts aid selection and must not imply that the full source or
connected context has been returned.

Do not add generic document list/get or a plain-read command in 0.2. This replaces
the earlier proposal for consecutive document reads through Mara; existing
structured item retrieval remains available. Location and revision handling
follow [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].

Verify that an actor can locate a narrative search hit and its linked item,
then read their complete original source with file tools, including content
larger than a search excerpt. The 0.1 boundary in
[[ADR-ALPHA-NARRATIVE-FILE-ACCESS]] remains unchanged.
:::

## Decisions needed before implementation planning

| Area | Open decision |
|---|---|
| Schema transition | Finalize the schema-format transition and migration guide. Reconcile the existing taxonomy with schema-owned guidance so each definition has one authority. Guidance shape, validation rules, and engineering relation definitions are settled in the designs below. |
| Discovery interface | The unified search and passage-navigation direction is settled in [discovery](discovery.mara.md). Finalize passage grouping, handles, link/anchor resolution, ranking, response bounds, wire fields, neighbour operation naming, and CLI/MCP migration policy. Do not reopen the separate document-search/read proposal. |

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
YAML examples using [[DES-FLAVOUR-AUTHORING-GUIDANCE]]. Explain how to preserve
custom flavours, fields, relations, and item identities while adding guidance
to every declared flavour, then verify schema and project validation through
CLI and MCP. Do not instruct users to reinitialize an existing project or
replace a customized schema with a bundled template.

Verification must demonstrate that a schema missing required guidance is
rejected by 0.2 and that following the guide makes it valid without unrelated
corpus changes. Finalize the schema-format transition before completing the
guide and implementing the new loader.
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

## Accepted 0.2 designs

:::mara design DES-FLAVOUR-AUTHORING-GUIDANCE
:mid: 01M231916PQP6XRY5PYCMMW8QE
:title: Store mandatory guidance directly in each flavour declaration
:satisfies: REQ-FLAVOUR-AUTHORING-GUIDANCE

The guidance keys are direct members of each flavour declaration alongside its
existing ID prefix, body requirement, and field declarations. There is no extra
`guidance` wrapper, and purpose continues to use `description`.

| Key | Type and validation |
|---|---|
| `description` | Required nonblank string. |
| `use_when` | Required sequence with at least one nonblank string. |
| `avoid_when` | Required sequence of nonblank strings; `[]` is valid when no meaningful exclusion applies. |
| `distinguish_from` | Required mapping from another declared flavour's name to a nonblank explanation; `{}` is valid when no confusable flavour applies. |

Omitting any key, supplying the wrong type, using blank entries, or naming an
unknown or the same flavour as a distinction target is invalid. Empty
collections explicitly express absence of applicable exclusions or distinctions;
authors must not add filler merely to satisfy validation.

Example guidance fragment inside the `requirement` declaration, assuming
`design` is also declared:

```yaml
description: An independently verifiable obligation.
use_when:
  - State behavior that must hold.
avoid_when:
  - Describe how a solution works.
distinguish_from:
  design: Describes how the obligation is satisfied.
```

CLI and MCP schema inspection expose these same declarations. Verify nonblank
content, missing keys, invalid value types, empty optional-content collections,
and unknown/self distinction targets through the real schema-loading workflow.
:::

:::mara design DES-ENGINEERING-RELATION-VOCABULARY
:mid: 01M231916ZSZ50Y2XCAFYF4JZJ
:title: Define the engineering template's additional typed relations
:satisfies: REQ-ENGINEERING-TRACEABILITY

The engineering template adds the following project-defined relations to the
existing vocabulary. Each edge is authored on its source and points to its
target; incoming views remain derived.

| Relation | Source flavours | Target flavours | Meaning |
|---|---|---|---|
| `verifies` | verification | requirement, design | Checks conformance to a specified obligation. |
| `validates` | verification | goal, scenario | Checks whether the intended outcome is achieved. |
| `evidences` | evidence | verification | Records a result from performing that verification. |
| `implements` | artifact | requirement, design | Identifies the implementation. |
| `affects` | risk | any flavour in the engineering template | Identifies knowledge exposed to that risk. |
| `mitigates` | requirement, design, decision, verification | risk | Specifies or provides a measure reducing that risk. |

Verification describes the check; evidence records its result. These links are
optional and do not require a complete trace chain. Existing relation meanings
and endpoints are unchanged. The engine continues to validate schema-declared
endpoints rather than hardcoding these names or introducing external or derived
source-code nodes.

Verify initialization from the engineering template, creation of representative
source/target items, relation addition, and incoming/outgoing retrieval through
CLI and MCP. Rejected endpoint combinations must preserve source files.
:::
