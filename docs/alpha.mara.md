# First alpha

The first alpha proves that one readable corpus can serve humans, agents,
requirements management, and project knowledge without imposing a generated
specification workflow.

:::mara goal GOAL-UNIFIED-PROJECT-KNOWLEDGE
:title: Keep project knowledge in one structured source of truth

Mara combines ordinary Markdown narrative with identifiable, typed, related,
and validated items. Requirements, design, decisions, and supporting knowledge
remain readable files rather than parallel documentation systems.
:::

:::mara goal GOAL-BOUNDED-AGENT-CONTEXT
:title: Let agents retrieve relevant knowledge without loading the full corpus

Agents discover, search, inspect, and traverse project knowledge incrementally.
Derived query results never become a second authoring authority.
:::

## Primary workflows

:::mara scenario SCN-START-STRUCTURED-PROJECT
:title: Start a project with structured documentation

The user initializes Mara in a new project, inspects its effective schema,
creates and relates requirements, validates the corpus, and retrieves the same
knowledge while implementing the project. This advances
[[GOAL-UNIFIED-PROJECT-KNOWLEDGE]].
:::

:::mara scenario SCN-AUTHOR-ITEM-FLEXIBLY
:title: Author an item through the most convenient path

An author creates a complete item through Mara, creates a scaffold and fills its
body manually, or writes an item directly and validates it. Tool-authored and
manually authored items follow the same schema and source format.
:::

:::mara scenario SCN-RETRIEVE-BOUNDED-KNOWLEDGE
:title: Retrieve bounded project knowledge

An author or agent searches with filters, fetches a selected item, and inspects
compact related-item summaries before retrieving any additional full bodies.
This advances [[GOAL-BOUNDED-AGENT-CONTEXT]].
:::

## Product contracts

:::mara requirement REQ-CANONICAL-SOURCE
:title: Git-tracked Mara documents remain canonical
:derives_from: SCN-START-STRUCTURED-PROJECT

Mara must read and write `*.mara.md` without making a database, generated
index, MCP response, CLI response, or other projection an authoring authority.
Direct reading, editing, and `ripgrep` remain supported workflows.
:::

:::mara requirement REQ-PROJECT-INITIALIZATION
:title: Initialize Mara in a current or named directory
:derives_from: SCN-START-STRUCTURED-PROJECT

`mara project init` initializes the current directory. An optional path is
created when absent or initialized when it is an existing directory. Global
`--project <path>` selects the initialization target when the positional path
is omitted; supplying both is rejected. Existing content is not
overwritten, and an existing Mara project is rejected. The default template is
`minimal`; `--template empty` creates no project flavours.
:::

:::mara requirement REQ-PROJECT-DISCOVERY
:title: Resolve one explicit project for each operation
:derives_from: SCN-START-STRUCTURED-PROJECT

Commands discover the nearest parent containing `.mara/project.toml`. Global
`--project <path>` overrides discovery. `mara mcp` uses the same resolution and
binds the running server to one project.
:::

:::mara requirement REQ-SCHEMA-DISCOVERY
:title: Make the effective project schema discoverable
:derives_from: SCN-START-STRUCTURED-PROJECT

Users and agents can retrieve the complete schema, list flavours or relations,
retrieve one declaration, and validate `.mara/schema.yaml`. Every flavour and
relation has a concise description suitable for discovery.
:::

:::mara requirement REQ-SURFACE-PARITY
:title: Expose the same operations through CLI and MCP
:derives_from: SCN-START-STRUCTURED-PROJECT

CLI and MCP must share operation semantics and domain results. CLI defaults to
human-readable output; global `--format json` returns stable agent-oriented
data equivalent to MCP structured results.
:::

:::mara requirement REQ-ITEM-CREATION
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
:::

:::mara requirement REQ-ITEM-INSERTION-SAFETY
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
:title: Add and remove typed relations explicitly
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`relation add <source> <relation> <target>` and the symmetric `relation remove`
resolve both items and validate the relation against schema source and target
flavours. Relations are authored once on the source; incoming backlinks are
derived.
:::

:::mara requirement REQ-PROJECT-VALIDATION
:title: Validate a selected item or the full project
:derives_from: SCN-START-STRUCTURED-PROJECT

`item validate <id>` checks one item in project context; `project validate`
checks the complete corpus. Validation covers project and schema configuration,
item syntax, known flavours, ID prefixes and uniqueness, required fields and
bodies, metadata, relation declarations and targets, and `[[ID]]` mentions.
Broken relations and mentions are errors. Mara reports every independently
discoverable diagnostic and skips only checks whose prerequisites are invalid.
:::

:::mara requirement REQ-ITEM-GET
:title: Retrieve one complete resolved item
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item get <id>` returns the item's flavour, ID, metadata, body, source location,
authored relations, and derived incoming relations. Human and structured output
represent the same result.
:::

:::mara requirement REQ-ITEM-SEARCH
:title: Search items deterministically with scope filters
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item search` performs deterministic case-insensitive text matching across ID,
title, body, and metadata. Exact flavour, field, relation, and path filters plus
a result limit narrow the scope. Results are compact summaries rather than full
item bodies. `item list` returns the same summary shape without a text query.
:::

:::mara requirement REQ-ITEM-RELATED
:title: Retrieve compact directly related items
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

`item related <id>` returns compact summaries of direct incoming and outgoing
neighbours and identifies each relation and direction. Direction, relation, and
flavour filters narrow results. Full bodies require explicit `item get` calls.
:::

## Alpha design

:::mara design DES-PROJECT-CONFIGURATION
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
:title: Use the global project option as an initialization target
:justifies: REQ-PROJECT-INITIALIZATION

Initialization accepts global `--project` as an alternative to its positional
target so automation can use one explicit-root option across bootstrap and
project-bound operations. The two forms conflict so target selection never
depends on precedence.
:::

:::mara decision ADR-STRICT-PROJECT-CONFIGURATION
:title: Reject unknown project configuration fields
:justifies: DES-PROJECT-CONFIGURATION

Project format 1 rejects unknown fields instead of ignoring them. This makes
misspellings and unsupported settings fail visibly rather than appear accepted,
and requires compatibility changes to update the documented format and loader
deliberately.
:::

:::mara design DES-MINIMAL-SCHEMA
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
:title: Object-operation command surface
:satisfies: REQ-SCHEMA-DISCOVERY
:satisfies: REQ-SURFACE-PARITY
:satisfies: REQ-ITEM-CREATION
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
| `project` | `init`, `validate` |
| `schema` | `get`, `list`, `validate` |
| `item` | `create`, `get`, `list`, `search`, `related`, `validate` |
| `relation` | `add`, `remove` |

`schema get` and `schema list` accept only the declared positional kinds
`flavour` and `relation`. MCP tools map to the same project-bound operations;
project initialization remains the CLI bootstrap operation.
:::

## Explicitly deferred

Alpha does not include structured item editing, moving, renaming, or deletion;
schema mutation commands; MID generation; persisted indexes or graph stores;
context profiles; fuzzy or semantic search; LSP integration; a complete
Markdown AST; or a graphical interface. Manual source editing followed by
validation remains the editing path.
