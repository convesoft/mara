# Guided authoring

Accepted scope for 0.2.0: a solo developer can start useful engineering
documentation, choose its vocabulary, and give an agent access to document
context as well as items. Schema format 2, mandatory flavour guidance,
all three bundled templates, and engineering traceability relations are
implemented starting with `0.2.0-alpha.0`. The current checkout also implements
unified discovery, reading, direct navigation, and reference-safe item mutations
under [the discovery contract](discovery.mara.md). The
[engineering profile](engineering-template.mara.md) extends the original template
with classifications, lifecycle policies and selected-scope checks.

Prioritize bundled templates and flavour guidance together, then useful
engineering relations and unified discovery. Diagnostic codes and severity are
outside 0.2; the [planned 0.3 traceability contract](traceability.mara.md)
introduces them for project-defined validation policy.

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

Offer an optional `engineering` template with the reusable engineering vocabulary
and incremental knowledge policies in [[DES-ENGINEERING-PROFILE]]. Keep `minimal`
as the default and retain `empty`. Initialization preserves
[[REQ-PROJECT-INITIALIZATION]] and creates no starter Markdown, product items or MIDs.

All templates install `.mara/project.toml` and `.mara/schema.yaml`.
Engineering additionally installs enabled `.mara/engineering-rules.yaml` and
request-local `.mara/engineering-checks.yaml` and `.mara/engineering-execution.yaml`.
Its configuration uses format 2; minimal and empty retain format 1. All schemas
use format 3.

Embed template source files in the executable. Generate configuration from the
destination name and selected template. Preserve all existing destination files;
on initialization failure remove only files created by that attempt. Bundled
template changes never silently rewrite previously initialized projects.
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

The engineering template connects stakeholder intent, scenarios, requirements,
design, decisions, implementation, verification, evidence and risk treatment.
Names, meanings and endpoints follow [[DES-ENGINEERING-RELATION-VOCABULARY]];
classifications, lifecycle policies and selected-scope coverage checks follow
[[DES-ENGINEERING-PROFILE]].

Authors add only meaningful links. Drafts can develop incrementally; acceptance
requires the agreed knowledge connections without forcing a complete implementation
or passing execution chain. Ordinary code checks need no duplicate verification
item. Existing schema, relation, rule and matrix operations implement the profile;
the engine does not hardcode engineering flavours or relation names.
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
:title: Generate project configuration and policy without starter documents
:justifies: REQ-ENGINEERING-TEMPLATE

Bundled templates generate editable project configuration and vocabulary.
Engineering additionally installs the accepted-knowledge policies and optional
coverage checks in [[DES-ENGINEERING-PROFILE]]. Flavour guidance belongs in the
schema and is exposed through schema inspection. Do not generate starter
Markdown, product items or a second guidance document.

The original schema-only boundary avoided premature process defaults.
The engineering review established a concrete incremental-authoring workflow:
drafts can remain incomplete, acceptance requires agreed knowledge connections,
and implementation/execution coverage is assessed separately. Shipping those
rules with the optional engineering profile makes that workflow usable without
changing the minimal default or previously initialized projects. Authors retain
ownership of document structure and can edit all generated assets.
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
:title: Define the engineering template's typed relationships
:satisfies: REQ-ENGINEERING-TRACEABILITY

The [engineering schema](../templates/engineering-schema.yaml) owns exact source
and target permissions and inverse aliases. Relations are project-owned vocabulary;
code endpoints and external references use the existing generic engine contracts.

| Relation / inverse | Meaning |
|---|---|
| contributes_to / supported_by | Scenario or requirement advances a goal |
| involves / involved_in | Scenario identifies a participating actor |
| refines / refined_by | More specific goal, requirement or design elaborates one of the same flavour |
| derives_from / source_of | Requirement or design originates from a scenario, requirement or authoritative artifact |
| satisfies / satisfied_by | Design defines a solution to a requirement |
| justifies / justified_by | Decision preserves rationale for a requirement, design or risk treatment |
| realizes / realized_by | Artifact provides a concrete realization of a requirement or design |
| implements / implemented_by | Code implements a requirement, design or verification method |
| checks / checked_by | Code defines a check of a requirement or design |
| verifies / verified_by | Verification method checks conformance to a requirement or design |
| validates / validated_by | Verification method checks achievement of a goal or scenario outcome |
| evidences / evidenced_by | Evidence records a verification execution result |
| affects / affected_by | Risk exposes knowledge to potential harm |
| mitigates / mitigated_by | Requirement, design, decision or verification provides a risk mitigation |
| depends_on / required_by | Source relies on the target |
| supersedes / superseded_by | Source replaces historical knowledge of the same flavour |
| sourced_from | Item cites an external originating document or authority |
| reported_at | Evidence locates an external supporting report |

`implements` and `checks` have code sources, and can be authored from items using
their inverse aliases. Artifact implementation uses `realizes`. Code links do not
establish execution or passing results. External links remain local-only terminal
references; they are not fetched.

When migrating an older engineering profile, reauthor artifact `implements`
edges as `realizes` before introducing code `implements`. Move same-flavour
decomposition from `derives_from` to `refines` only after reviewing its meaning.
Existing self-hosting `code_implements` and `code_verifies` names are migrated
separately with their code markers and item-authored inverses; a bundled template
change never rewrites a project's vocabulary.
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
