# Guided authoring

Accepted scope for 0.2.0: a solo developer can start useful engineering
documentation, choose its vocabulary, and give an agent access to document
context as well as items. Schema format 2, mandatory flavour guidance,
all three bundled templates, and engineering traceability relations are
implemented starting with `0.2.0-alpha.0`. The current checkout also implements
unified discovery, reading, direct navigation, and reference-safe item mutations
under [the discovery contract](discovery.mara.md).

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
and finds a structured item, section, or ordinary Markdown block, including
content in documents without items. Starting from a Markdown block's explicit
mention, the actor inspects the referenced requirement and then its direct
connections to designs or decisions, choosing each subsequent step.

Search and neighbour results expose source locations and connection meaning.
The actor reads each selected node through Mara, continuing bounded reads
until the needed content is complete. This workflow does not require client
filesystem access when a neighbour is a Markdown block, section, or document.
Authors need not turn narrative into items. Discovery, reading, and direct
navigation have equivalent CLI and MCP behavior; richer graph analysis and
code-symbol extraction are later work.
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

Maintain the `minimal`, `empty`, and `engineering` schema templates as source
files embedded in the executable, without requiring a runtime template directory.
Generate project configuration programmatically, deriving the project name from
the destination directory. Changing a bundled schema template must not silently
rewrite schemas in projects already initialized from it.
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

CLI and MCP must discover matching items, sections, and ordinary Markdown
blocks through one bounded search surface in the selected project's canonical
documents, including narrative-only documents. Move the CLI entry point to
`mara search`. Return explicit result kinds, source locations, and continuation
when more matches remain. Do not add a separate document-search operation.

Markdown blocks are first-class discovery nodes without requiring authored item
IDs, MIDs, or flavours. Their explicit connections follow
[[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]]. Result boundaries, filters, source
reading, and remaining interface details follow
[[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].

Expose structural membership under [[DES-DOCUMENT-STRUCTURE]] so an actor can
identify the containing section or document and navigate from it.

Verify mixed item/section/block results, narrative-only documents, path and
item-only filters, bounded continuation, and absence of duplicated item-body
hits through CLI and MCP.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-READ
:mid: 01M1XSKPPTZCGKHDSYW0SKMB6B
:title: Read discovered nodes through bounded Mara retrieval
:derives_from: SCN-READ-DOCUMENT-CONTEXT

In 0.2, an actor can read an item, section, Markdown block, or document
identified by an item ID/MID or discovery handle through CLI and MCP. Return
node kind, source location, structural context, and bounded consecutive
content, with item metadata when applicable. Provide explicit continuation
that reconstructs complete content without gaps, including oversized nodes.

Discovery excerpts aid selection and may omit content; they do not replace
consecutive retrieval. Enumerate direct connections through the separate
related operation. Client filesystem access is optional throughout discovery,
reading, and successive direct navigation. Command and handle contracts follow
[[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].

Verify search, node read, direct-neighbour lookup, and neighbour read through
both CLI and MCP, including a narrative-only document and item, section, and
block targets. Verify complete reconstruction for content exceeding a response
budget and rejection of stale handles/continuation. This extends 0.2 only;
[[ADR-ALPHA-NARRATIVE-FILE-ACCESS]] retains the 0.1 boundary.
:::

## Implementation planning

Schema format 2, schema-owned flavour guidance, and the taxonomy transition are
accepted in [[DES-FLAVOUR-AUTHORING-GUIDANCE]]. Follow the short
[0.2 migration guide](migration-0.2.mara.md).

Discovery commands, node reading, handles, link resolution, relation names,
ranking, response fields/bounds, and CLI/MCP migration are settled in
[discovery](discovery.mara.md).

Delivery tickets reference the accepted designs, decisions, and requirements;
verification belongs with each implemented outcome.

## Decisions and migration

:::mara requirement REQ-FLAVOUR-GUIDANCE-MIGRATION
:mid: 01M22ZQ349GZHVT2F2R1MRX90G
:title: Document migration to mandatory flavour guidance
:derives_from: REQ-FLAVOUR-AUTHORING-GUIDANCE

The 0.2.0 release must identify mandatory flavour guidance as a breaking schema
change and link a canonical migration guide from its release notes. Version the
incompatible persisted schema contract independently of the application version.

The guide must describe schema format 1 to 2 migration and before/after
YAML examples using [[DES-FLAVOUR-AUTHORING-GUIDANCE]]. Explain how to preserve
custom flavours, fields, relations, and item identities while adding guidance
to every declared flavour, then verify schema and project validation through
CLI and MCP. Do not instruct users to reinitialize an existing project or
replace a customized schema with a bundled template.

Verification must demonstrate that a schema missing required guidance is
rejected by 0.2 and that following the guide makes it valid without unrelated
corpus changes. Follow the transition in [[DES-FLAVOUR-AUTHORING-GUIDANCE]]
and the [0.2 migration guide](migration-0.2.mara.md).
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
Use schema format 2 to distinguish this required-guidance contract from
format 1. Make the schema authoritative for flavour descriptions and selection
guidance, retaining broader policy in the taxonomy. This prevents duplicated
definitions from drifting between schema inspection and prose documentation.
The implementation and schema migration belong to 0.2; the 0.1 release retains
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

Starting with 0.2, schemas use `format_version: 2`. Bundled templates
emit version 2, and existing version-1 schemas require explicit migration;
reject them with a migration diagnostic rather than silently upgrading them.
This change does not alter document or project-configuration format versions.

The selected project's schema owns flavour descriptions and selection guidance.
During the 0.2 migration, move duplicated flavour definitions from the
self-hosting taxonomy into schema guidance, preserving their useful meaning.
Keep broader authoring policy and links in the taxonomy; do not maintain a
second set of flavour definitions. Schema inspection exposes the authoritative
declarations. The 0.1 release retains its current schema and taxonomy.

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

:::mara evidence EVD-02-PACKAGED-WORKFLOW
:mid: 01M2B0SCR7CMFF4NF129FJKJX5
:title: Packaged 0.2 workflow acceptance evidence

Verified on 2026-09-12 for MARA-55 against implementation commit
`0b8fc05e917a3a2bda46ff30e5560c105bc2f36f`, using the expanded
[`scripts/smoke-npm.sh`](../scripts/smoke-npm.sh) in this change. The executable
reports `0.2.0-alpha.0`; that version alone does not identify these unreleased
changes. Host: Linux x86_64, Rust 1.97.1, Node 26.7.0, npm 11.19.0.

From this checkout, reproduce with:

```sh
cargo build --locked --release
scripts/smoke-npm.sh target/release/mara
```

The script builds local dispatcher/native npm tarballs, installs them with a
fresh npm cache into a temporary directory, and launches the installed CLI and
stdio MCP outside the repository. The acceptance calls use a PATH containing
only Node; attempts to execute `cargo`, `rustc`, `rustup`, or a global `mara`
return ENOENT. The installed skill matches the source packaged for this run.
All five `PASS` checkpoints completed:

| Accepted workflow | Observed result |
|---|---|
| [[SCN-START-ENGINEERING-KNOWLEDGE]] and [[SCN-CHOOSE-KNOWLEDGE-FLAVOUR]] | Engineering initialization generated only project configuration and schema. All 11 flavour declarations, including selection guidance, agreed through CLI/MCP. CLI created a requirement and design with `satisfies`; MCP created verification, added `verifies`, and exposed its incoming connection. |
| [[SCN-READ-DOCUMENT-CONTEXT]] | Mixed search returned item, section, and block nodes. Successive get/related/get calls followed narrative to requirement to verification. Relative links from a narrative-only document resolved to another document and section, with incoming backlinks. Parent/child navigation selected sibling prose; every search/related continuation page agreed through CLI/MCP. |
| [[REQ-DOCUMENT-CONTEXT-READ]] | Oversized Unicode block, section, and document reads reconstructed exact source bytes across multiple pages. Every domain page stayed within 65,536 bytes. |
| [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]] | A different document's edit preserved structural handles but invalidated search/get/related cursors. Editing the containing document invalidated get/related handles. Both transports rejected stale input with recovery guidance; rediscovery succeeded. |
| [[REQ-FLAVOUR-GUIDANCE-MIGRATION]] | The migration guide's custom `term` schema rejected format 1 and rejected a version-only change lacking guidance. Adding guidance in place preserved declarations, repeated aliases, `clarifies` relations, IDs/MIDs, document bytes, and configuration bytes. CLI/MCP schema and project validation passed; subsequent MCP item creation with custom fields and a relation succeeded. |

The migration fixture uses Mara-generated identities, then stages the guide's
format-1 schema over those real files before migrating it. It does not execute
an older Mara binary. These are locally assembled packages of the specified
source, not verification of a published npm release. This execution covers
Linux x64 only; the existing release workflow runs the same smoke script on its
other supported targets. Feature-level tests remain with their implementations.
:::
