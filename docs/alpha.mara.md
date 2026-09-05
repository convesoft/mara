# First alpha

The first alpha proves that one readable corpus can serve humans, agents,
requirements management, and project knowledge without imposing a generated
specification workflow.

:::mara goal GOAL-UNIFIED-PROJECT-KNOWLEDGE
:mid: 01M1PXP2KGBN9S4G5PC0SAE5P6
:title: Keep project knowledge in one structured source of truth

Mara combines ordinary Markdown narrative with identifiable, typed, related,
and validated items. Requirements, design, decisions, and supporting knowledge
remain readable files rather than parallel documentation systems.
:::

:::mara goal GOAL-BOUNDED-AGENT-CONTEXT
:mid: 01M1PXP2KGMDJHQGF2MCTP7TED
:title: Let agents retrieve relevant knowledge without loading the full corpus

Agents discover, search, inspect, and traverse project knowledge incrementally.
Derived query results never become a second authoring authority.
:::

## Primary workflows

:::mara scenario SCN-START-STRUCTURED-PROJECT
:mid: 01M1PXP2KGMG0M2HVW68FS0EJW
:title: Start a project with structured documentation

The user initializes Mara in a new project, inspects its effective schema,
creates and relates requirements, validates the corpus, and retrieves the same
knowledge while implementing the project. This advances
[[GOAL-UNIFIED-PROJECT-KNOWLEDGE]].
:::

:::mara scenario SCN-AUTHOR-ITEM-FLEXIBLY
:mid: 01M1PXP2KGMN7M1S7ND2AGM8ZK
:title: Author an item through the most convenient path

An author creates a complete item through Mara, creates a scaffold and fills its
body manually, or writes an item directly and validates it. Tool-authored and
manually authored items follow the same schema and source format.
:::

:::mara scenario SCN-RETRIEVE-BOUNDED-KNOWLEDGE
:mid: 01M1PXP2KGTZWMSGFSKY6CCKW4
:title: Retrieve bounded project knowledge

An author or agent searches with filters, fetches a selected item, and inspects
compact related-item summaries before retrieving any additional full bodies.
This advances [[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

:::mara scenario SCN-ONBOARD-MARA-AGENT
:mid: 01M1PXP2KGEBFRGNFRZ89V2JMS
:title: Give an agent access to Mara project knowledge

A user connects an installed Mara executable to the agent as an MCP server and
installs its skill separately. A compatible client may instead install the
optional complete Agent Plugin. The agent initializes an explicit project when
needed, inspects its schema, and performs bounded operations against one
selected project through MCP. This advances [[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

## Product contracts

:::mara requirement REQ-CANONICAL-SOURCE
:mid: 01M1PXP2KGFTACAY742WPSWRK7
:title: Git-tracked Mara documents remain canonical
:derives_from: SCN-START-STRUCTURED-PROJECT

Mara must read and write `*.mara.md` without making a database, generated
index, MCP response, CLI response, or other projection an authoring authority.
Direct reading, editing, and `ripgrep` remain supported workflows.
:::

:::mara requirement REQ-PROJECT-INITIALIZATION
:mid: 01M1PXP2KG7SDH3FRPXK9EN06C
:title: Initialize Mara in a current or named directory
:derives_from: SCN-START-STRUCTURED-PROJECT

`mara project init` initializes the current directory. An optional path is
created when absent or initialized when it is an existing directory. Global
`--project <path>` selects the initialization target when the positional path
is omitted; supplying both is rejected. Existing content is not
overwritten, and an existing Mara project is rejected. The default template is
`minimal`; `--template empty` creates no project flavours. MCP `project_init`
provides the same operation with an optional `template`. An unbound server call
requires an absolute `project` path; a server started with `--project` uses its
bound target and rejects a request-level `project` override.
:::

:::mara requirement REQ-PROJECT-DISCOVERY
:mid: 01M1PXP2KG99P47TB3HH0A9635
:title: Resolve one explicit project for each operation
:derives_from: SCN-START-STRUCTURED-PROJECT

Commands discover the nearest parent containing `.mara/project.toml`. Global
`--project <path>` overrides discovery. `mara mcp` starts without resolving a
project. Each project-bound MCP tool accepts an optional absolute `project`
path; when absent, it discovers from the server execution directory. Starting
the server as `mara mcp --project <path>` binds it to that selection, and tools
must then omit `project`. Each operation resolves exactly one project.
:::

:::mara requirement REQ-SCHEMA-DISCOVERY
:mid: 01M1PXP2KGPMYWSV20VA6TP3R6
:title: Make the effective project schema discoverable
:derives_from: SCN-START-STRUCTURED-PROJECT

Users and agents can retrieve the complete schema, list flavours or relations,
retrieve one declaration, and validate `.mara/schema.yaml`. Every flavour and
relation has a concise description suitable for discovery.
:::

:::mara requirement REQ-SURFACE-PARITY
:mid: 01M1PXP2KGA6GZQB9MCYMYVNJA
:title: Expose the same operations through CLI and MCP
:derives_from: SCN-START-STRUCTURED-PROJECT

CLI and MCP must share operation semantics and domain results. CLI defaults to
human-readable output; global `--format json` returns stable agent-oriented
data equivalent to MCP structured results.
:::

:::mara requirement REQ-PORTABLE-AGENT-ONBOARDING
:mid: 01M1PXP2KG77N00V05NSNKTRN0
:title: Package Mara for portable agent onboarding
:derives_from: SCN-ONBOARD-MARA-AGENT

The supported agent onboarding path registers an installed Mara executable as
an MCP server and installs the Mara skill separately. The existing
`@convesoft/mara` package also contains an Agent Plugins 1.0 manifest, the same
skill, and stdio MCP configuration as an optional complete-plugin convenience.
The skill guides agents to select an explicit project and discover its schema
before operating. Neither route creates or modifies project `AGENTS.md`.
Complete-plugin compatibility is client-managed and is not a release gate.
:::

:::mara requirement REQ-ITEM-CREATION
:mid: 01M1PXP2KGSJHD00W32AGQ3YVT
:title: Create a complete item or scaffold in an explicit document
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`item create` requires flavour, human ID, destination file, title, and all
schema-required metadata. It creates a missing file only when its parent exists
and otherwise appends the item. The destination must be selected by the
project's content include, Git ignore, and directory-symlink discovery rules.
`--line N` inserts immediately before the one-based line `N`; `line_count + 1`
means end of file. Body input accepts an inline value or `-` for standard input.

When a required body is omitted, creation succeeds as an incomplete scaffold
and reports `complete: false` with `body` missing. Validation continues to
reject the item until its body is filled. An optional body may remain empty.

Mara generates the item's MID, writes it as exactly one `:mid:` entry
immediately after the item opener, and returns it with the creation result.
Callers cannot provide, copy, or update a MID through item creation.
:::

:::mara requirement REQ-DURABLE-ITEM-IDENTITY
:mid: 01M1PXP2KG5R46BBV7ZGQE6XGB
:title: Give every item a durable machine identity
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

Every item has one repository-wide immutable MID in addition to its
human-readable ID. A MID is a raw canonical uppercase 26-character ULID with no
prefix. Human IDs remain readable handles and authored relation targets, while
identity-dependent resolution can use either an exact MID or an exact human ID
deterministically. Duplicate MIDs and duplicate human IDs are invalid even when
the other identity differs.
:::

:::mara requirement REQ-MID-BACKFILL
:mid: 01M1PXP2KG07XC4AX4G6X7BSNC
:title: Backfill missing item identities deliberately
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`project mid backfill` and MCP `project_mid_backfill` add generated MIDs to
every legacy item that lacks one. Backfill preserves existing source content and
existing MIDs, writes each new `:mid:` immediately after its item opener, and
makes no source changes when its validation preflight fails. Normal reads never
backfill automatically.
:::

:::mara requirement REQ-ITEM-INSERTION-SAFETY
:mid: 01M1PXP2KGKT8ET242DR0YHV9S
:title: Insert items without corrupting document structure
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

An explicit insertion point must be outside every existing Mara item but may
split ordinary narrative. Mara ensures at least one blank line around the new
item when adjacent content exists, avoids accumulating blank lines, validates
the resulting source structure, and replaces the source atomically only on
success. Body input that would close the created item or introduce another item
is rejected. An explicitly scaffolded missing body remains a semantic
validation error without making the insertion structurally unsafe.
:::

:::mara requirement REQ-RELATION-MUTATION
:mid: 01M1PXP2KGENM6H6CP04STA54Q
:title: Add and remove typed relations explicitly
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`relation add <source> <relation> <target>` and the symmetric `relation remove`
resolve both items and validate the relation against schema source and target
flavours. Relations are authored once on the source; incoming backlinks are
derived.
:::

:::mara requirement REQ-PROJECT-VALIDATION
:mid: 01M1PXP2KGWKQRXBB29DX5D7G1
:title: Validate a selected item or the full project
:derives_from: SCN-START-STRUCTURED-PROJECT

`item validate <id>` checks one item in project context; `project validate`
checks the complete corpus. Validation covers project and schema configuration,
item syntax, known flavours, ID prefixes and uniqueness, MID presence, MID
format, MID uniqueness, required fields and bodies, metadata, relation
declarations and targets, and `[[ID]]` mentions. Broken relations and mentions
are errors. Mara reports every independently discoverable diagnostic with an
actionable source location and skips only checks whose prerequisites are
invalid.
:::

:::mara requirement REQ-ITEM-GET
:mid: 01M1PXP2KGGWT0WRKDJXC5NS4G
:title: Retrieve one complete resolved item
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item get <id-or-mid>` returns the item's flavour, human ID, MID, metadata,
body, source location, authored relations, and derived incoming relations. Human
and structured output represent the same result. Relation entries include the
relation name and a compact summary containing both identities for the directly
related item.
:::

:::mara requirement REQ-ITEM-SEARCH
:mid: 01M1PXP2KGKART3Y9XWADR46F5
:title: Search items deterministically with scope filters
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item search <text>` performs deterministic Unicode case-insensitive keyword
matching across ID, title, body, and metadata keys and values. Every distinct
query term must occur as a complete term in at least one searchable value;
terms may occur in different values, in any order, and without adjacency.
Partial terms, spelling correction, stemming, synonym expansion, and relevance
ranking are not supported. Both query commands accept repeatable
`--flavour <name>`, `--field <key=value>`, `--relation <name>`, and
`--path <project-relative-path>` filters plus `--limit <count>`.
Path filters normalize `.` components and reject absolute paths or `..`.
Relation filters match authored relation names. Values within one filter or one
field key combine with OR; distinct filter categories and field keys combine
with AND. The limit applies last. Results follow document-path and source order
and contain only ID, MID when present, flavour, title, source path, and start
line. `item list` returns the same compact summary shape without a text query.
:::

:::mara requirement REQ-ITEM-RELATED
:mid: 01M1PXP2KG97XHSEEB4KCZVTAP
:title: Retrieve compact directly related items
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item related <id-or-mid>` returns compact summaries of direct incoming and
outgoing neighbours and identifies each relation and direction. Direction,
relation, and flavour filters narrow results. `--direction` accepts `incoming`
or `outgoing` and defaults to both; `--relation <name>` and `--flavour <name>`
are repeatable. Outgoing results precede incoming results, with each group in
corpus and authored relation order. Full bodies require explicit `item get`
calls.
:::

## Alpha design

:::mara design DES-PROJECT-CONFIGURATION
:mid: 01M1PXP2KGYT8EAYRHXYSMZY0V
:title: Minimal project configuration
:satisfies: REQ-PROJECT-INITIALIZATION
:satisfies: REQ-PROJECT-DISCOVERY

`.mara/project.toml` is the project-root marker. Initialization writes draft
format 1, derives the project name from the root directory, and selects the
default schema and all Mara documents:

```toml
format_version = 1

[project]
name = "example"
schema = ".mara/schema.yaml"

[content]
include = ["**/*.mara.md"]
```

Schema paths and content patterns are project-relative. Alpha respects
`.gitignore`, skips directory symlinks, and builds query state in memory without
configurable policies or a persisted index. Project and schema format versions
start at draft version 1 and change only for incompatible persisted contracts,
independently from application SemVer.
:::

:::mara decision ADR-EXPLICIT-INIT-TARGET
:mid: 01M1PXP2KGRT31XDJG6KEJZEDB
:title: Use the global project option as an initialization target
:justifies: REQ-PROJECT-INITIALIZATION

Initialization accepts global `--project` as an alternative to its positional
target so automation can use one explicit-root option across bootstrap and
project-bound operations. The two forms conflict so target selection never
depends on precedence.
:::

:::mara decision ADR-STRICT-PROJECT-CONFIGURATION
:mid: 01M1PXP2KG1Z4GCKD2545KN1BJ
:title: Reject unknown project configuration fields
:justifies: DES-PROJECT-CONFIGURATION

Project format 1 rejects unknown fields instead of ignoring them. This makes
misspellings and unsupported settings fail visibly rather than appear accepted,
and requires compatibility changes to update the documented format and loader
deliberately.
:::

:::mara design DES-MINIMAL-SCHEMA
:mid: 01M1PXP2KGT0F06H7DCBNXPRNH
:title: Minimal default schema
:satisfies: REQ-SCHEMA-DISCOVERY
:satisfies: REQ-PROJECT-VALIDATION

The default `minimal` template writes `.mara/schema.yaml` draft format 1 with
`scenario`, `requirement`, `design`, and `decision`; `empty` writes empty
`flavours` and `relations` maps. A flavour declares its description, ID prefix,
body requirement, and custom fields. Custom fields are flat string, integer,
number, boolean, or enum values with required and repeatable constraints. The
structural names `mid`, `flavour`, `id`, `title`, and `body` cannot be custom
fields. Enum members are non-empty, unique, and have no surrounding whitespace.
Relation names cannot be structural names or match a custom field declared by
any allowed source flavour.

Initial relations are `derives_from`, `depends_on`, `satisfies`, `justifies`,
and `supersedes`:

| Relation | Source | Target |
|---|---|---|
| `derives_from` | requirement or design | scenario or requirement |
| `depends_on` | any item | any item |
| `satisfies` | design | requirement |
| `justifies` | decision | requirement or design |
| `supersedes` | any item | item of the same flavour |

Alpha derives backlinks but does not support inverse authoring, cardinality,
acyclicity, external targets, nested fields, defaults, regex constraints,
computed fields, or custom rules.
:::

:::mara design DES-COMMAND-SURFACE
:mid: 01M1PXP2KG6N0GDT9FW7231DB4
:title: Object-operation command surface
:satisfies: REQ-SCHEMA-DISCOVERY
:satisfies: REQ-SURFACE-PARITY
:satisfies: REQ-ITEM-CREATION
:satisfies: REQ-MID-BACKFILL
:satisfies: REQ-RELATION-MUTATION
:satisfies: REQ-ITEM-GET
:satisfies: REQ-ITEM-SEARCH
:satisfies: REQ-ITEM-RELATED

CLI commands use `mara <object> <operation>`. The alpha objects are `project`,
`schema`, `item`, and `relation`; `mara mcp` starts the stdio MCP surface.
`get` resolves one object, `list` enumerates with exact filters, `search` adds a
text query, and mutations use explicit verbs such as `create`, `add`, and
`remove`.

Item creation uses `item create <flavour> <id> <file> --title <title>` with
repeatable `--field <key=value>`, optional `--body <text|->`, and optional
`--line <N>`. The destination file is project-relative; repeat `--field` only
for schema-repeatable metadata.

| Object | Operations |
|---|---|
| `project` | `init`, `validate`, `mid backfill`, `transaction rollback` |
| `schema` | `get`, `list`, `validate` |
| `item` | `create`, `move`, `get`, `list`, `search`, `related`, `validate` |
| `relation` | `add`, `remove` |

`schema get` and `schema list` accept only the declared positional kinds
`flavour` and `relation`. Each project-bound operation maps to an MCP tool by
joining the object and operation with `_`, from `project_validate` through
`relation_remove`; initialization maps to `project_init`. Project-bound MCP
tools add an optional absolute `project` path to the shared operation input.

Item movement and explicit transaction rollback follow [[DES-ITEM-MOVEMENT]].
MCP `item_move` accepts `reference`, `file`, and optional `line`.

CLI `--format json` and MCP `structuredContent` serialize the same domain result.
Item collections use `{ "items": [...] }`. Project and item validation return
`valid`, `project`, `target`, and `diagnostics`; an invalid target is a completed
result, with each diagnostic providing `scope`, optional `path` and `line`, and
`message`. Invocation failures use `{ "error": { "message": ... } }` in CLI
JSON and a caller-visible MCP tool error.
:::

:::mara design DES-DURABLE-ITEM-IDENTITIES
:mid: 01M1PXP2KGC400ND3WXDFZJE88
:title: Store machine identities as structural metadata
:satisfies: REQ-DURABLE-ITEM-IDENTITY
:satisfies: REQ-MID-BACKFILL
:satisfies: REQ-SURFACE-PARITY

The in-memory item projection retains both the authored human ID and the
generated MID. Query and mutation operations resolve item handles by exact MID
when the handle is a canonical MID, otherwise by exact human ID. Compact and
resolved item structures include both identities when a MID is present.

Resolved relation traversal resolves target handles to item identities
internally. Relation metadata remains authored as human-readable IDs by
operation output, and relation mutation writes the target item's human ID even
when the caller supplied its MID.
:::

:::mara design DES-OPERATION-PROJECT-CONTEXT
:mid: 01M1PXP2KG1T44F3T2E5YR4YX2
:title: Resolve project context at the operation boundary
:satisfies: REQ-PROJECT-DISCOVERY
:satisfies: REQ-SURFACE-PARITY

Public operation wrappers select a project from an explicit path or the
execution directory, resolve its current project context, and then invoke the
operation against that resolved context. MCP request selection is absolute and
takes effect only when the server was not started with `--project`; a bound
server rejects request-level selection. Resolution occurs for each call, so
source and schema changes remain visible without server-side project caches.

This boundary remains one project per operation. It does not define nested
project precedence, workspace aggregation, or cross-project operations.
:::

:::mara design DES-PORTABLE-AGENT-PLUGIN
:mid: 01M1PXP2KGAC399SK9RG2SX763
:title: Portable Agent Plugin package
:satisfies: REQ-PORTABLE-AGENT-ONBOARDING

The main npm package is also an optional Agent Plugin root. It contains the
Agent Plugins 1.0 `plugin.json`, discovers `skills/mara/SKILL.md`, and declares
the packaged plugin launcher as a stdio server in `mcp.json`. The launcher uses
an installed matching native package when available or runs the package's exact
version through `npx`. Its MCP process relies on request-level absolute project
selection instead of assuming that the plugin installation directory is a Mara
project. Client-specific complete-plugin installation behavior is outside the
supported manual MCP-plus-skill contract.
:::

:::mara design DES-DETERMINISTIC-KEYWORD-SEARCH
:mid: 01M1PXP2KGHS11B9YGCD35EP8S
:title: Replaceable deterministic keyword matching
:satisfies: REQ-ITEM-SEARCH

Search analyzes query text and each searchable item value with the same pipeline:
NFC normalization, Unicode full default case folding, NFC normalization, and
Unicode Standard Annex #29 word segmentation. Each distinct query term is
matched as a complete term.

An item matches when every query term occurs in at least one of its searchable
values. Terms may occur in different values, in any order, and without
adjacency. The matcher does not stem terms, expand synonyms, correct spelling,
or rank results.

Matching remains an internal projection over the loaded corpus. It writes no
index or search-specific project data and returns matching items in corpus
order, so a later search engine can replace it without migrating authored
project knowledge.
:::

## Explicitly deferred

Alpha does not include structured item editing, moving, renaming, or deletion;
schema mutation commands; persisted indexes or graph stores; fuzzy or semantic
search; LSP integration; a complete Markdown AST; or a graphical interface.
Manual source editing followed by validation remains the editing path.
