# Code traceability

This document owns production code association and symbol identity semantics.

:::mara requirement REQ-CODE-TRACEABILITY
:mid: 01M3EQAFT3DPZ2VF1GWZBX2T6Y
:title: Resolve code associations as typed relations

A project can declare code-to-item relations, navigate the resulting source
associations, and mutate item-authored inverse assertions through relation
commands. Source code comments remain directly authored in code. Missing,
ambiguous and unsupported code targets produce source-located diagnostics. A
source association does not establish passing test evidence. Symbol links use exact
indexer identities: body edits, overload ordering, and package version bumps
must not select another symbol. Renames, moves, or deletion may break links;
Mara never falls back to a matching name or position. Language indexers own
overload distinctions; declarations sharing one implementation may share an identity.
:::

:::mara design DES-CODE-TRACEABILITY
:mid: 01M3EQAJBYC0Q2TAVD3KM6E11H
:title: Resolve code endpoints through pluggable SCIP indexers
:satisfies: REQ-CODE-TRACEABILITY

Schema format 3 accepts optional `code_source: true` on a directed relation.
Such a declaration has `source: []`, a nonempty item-flavour `target`, and an
optional `inverse` for authoring from the item. It cannot be symmetric,
same-flavour, external, or code-to-code. It may declare incoming cardinality
on eligible item targets; outgoing and symmetric cardinality and acyclic
policies are invalid. A relation without `code_source` retains
its current item/external behaviour. All names remain project-declared. For
example, Mara's self-hosting project declares:

```yaml
relations:
  code_implements:
    description: The code implements the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: implemented_by_code
  code_verifies:
    description: The code defines a check of the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: verified_by_code
```

The canonical edge is `(relation, code endpoint, item MID)`. Code is a built-in
node kind in the disposable relation graph, alongside item and external nodes.
Each kind supplies its own graph identity; only items have persisted MIDs and
schema-defined flavours. Code identity is the exact project-relative
file path plus the configured language name and exact SCIP descriptor, or just the path for a
file-only endpoint. Identical assertions from either side count as one edge
with distinct source occurrences. Code links are structural associations;
`verifies` identifies a check definition and never claims a passing execution.
Incoming cardinality counts distinct canonical code-to-item edges, including
marker and item-authored assertions, at each eligible item. It does not count
multiple occurrences of one edge twice.

## Authored forms

An item may assert the inverse in metadata or a typed inline reference:

```markdown
:implemented_by_code: code:src/graph_constraints.rs::rust::graph_constraints/evaluate().

Implemented by [[implemented_by_code:code:src/graph_constraints.rs::rust::graph_constraints/evaluate().]].
```

The target grammar is `code:<project-relative-path>[::<language>::<descriptor>]`.
Paths use `/`, contain only ordinary relative components, and cannot escape the
project through `..` or a symlink. Matching is case-sensitive. A file-only target
resolves to an existing regular file without an indexer or grammar. Symbol paths
must be reported by the configured indexer. Descriptors retain SCIP's punctuation
and disambiguators, omitting scheme and all package metadata. Whitespace and
`%`, `[`, `]`, `<`, `>`, backslash and `|` are percent-escaped using
uppercase UTF-8 byte values. Other characters, including backticks, are preserved
in metadata and typed inline references. For example,
`` code:service.ts::typescript::`service.ts`/parse(). `` represents the descriptor
`` `service.ts`/parse(). ``. These spellings are canonical, not URL aliases.
No authored byte span or line number is part of the identity.

A source comment marker is `@mara <canonical-relation> <item-ID-or-MID>` on
its own comment line. Tree-sitter identifies comment nodes and source spans;
one shared parser searches only their text for markers. It does not scan raw
source as text. The relation must declare `code_source: true` and allow
the target item's flavour. Rust `//`, `///`, and block comments; Python `#`;
and JavaScript/TypeScript `//` and block comments are supported. A marker
in a leading group of query-captured comments attaches to the following named
declaration through its existing modifier/wrapper boundary. All markers in the
group share that owner; ordinary and documentation comments may intervene,
separated only by whitespace, including blank lines. Attachment cannot cross
statements, unrelated declarations, unsupported constructs or lexical body
boundaries. Otherwise a marker inside a declaration body attaches to the deepest
containing named declaration; a top-level marker with no attached declaration
attaches to the file. Ambiguous or unsupported ownership remains an error.
Each marker retains its own original source span. Grouping does not extend
returned declaration content to include leading comments; existing modifier and
wrapper content coverage is unchanged. Comment recognition remains query-driven;
Mara applies the shared attachment rules.

## Indexer and grammar boundary

Mara contains no compiled language integrations. Projects configure one
`[[code.languages]]` entry per integration in `.mara/project.toml` format 3.
Each entry combines a SCIP command with optional Tree-sitter WebAssembly assets.
Mara owns invocation, standard SCIP protobuf reading, marker parsing, schema/item
resolution, graph identity and navigation.
Each indexer owns language support, project discovery, compilation requirements
and descriptor generation. Adding a language does not require a Mara rebuild.

```toml
format_version = 3

[[code.languages]]
name = "rust"
command = ["rust-analyzer", "scip", ".", "--output", "{output}"]
position_encoding = "utf8"

# Optional: declaration content and source comment attachment.
extensions = ["rs"]
grammar = ".mara/code/rust.wasm"
query = ".mara/code/rust.scm"
```

Each unique snake_case language name is persisted in links; renaming it breaks
those links. `command` is an executable and argument array, with exactly one
standalone `{output}` argument. Mara replaces that argument with a temporary
output path and runs the command from the project root without an implicit shell.
The `command` is required. Configure `extensions`, `grammar` and `query` together,
or omit all three for indexing without Tree-sitter. Install the executable and
its language dependencies separately. Mara does not download indexers, compile
grammars, or contain language-specific command defaults.
Only enable commands trusted by the project: they can run compiler/build tooling
and have the permissions and network access of the Mara process.

Mara invokes every configured indexer whenever it loads code for a corpus
operation, including validation and reads; users need no pre-indexing step.
It consumes fresh output and keeps no persistent SCIP cache. Nonzero exit,
missing/malformed output, a different indexed project root, invalid source ranges,
or changed project inputs during execution produce incomplete-validation errors;
normal operations fail instead of returning a partial graph. Command diagnostics
identify the indexer without exposing its raw stdout/stderr. Input snapshots use
unignored regular project files, excluding Mara mutation bookkeeping and staging
files. Indexer-generated build artifacts should follow project ignore rules.

Document positions follow SCIP's declared encoding. For older indexers that omit
it, configure their documented `position_encoding` (`utf8`, `utf16`, or `utf32`).
Without either declaration, ASCII is unambiguous; non-ASCII source is rejected.
Only global definition occurrences are linkable. SCIP `local N` identities are
unstable and excluded. Two different full symbols that collapse to the same
file/indexer/descriptor cause an error. Multiple declarations of one full symbol
share one endpoint, including TypeScript overload signatures and implementation.
Independently implemented overloads remain distinct when the indexer supplies
different descriptors. Mara does not invent signatures or choose among candidates.
The guarantee relies on the indexer's descriptor stability: upstream identity
reuse cannot be detected without persistent history. Changing indexer semantics
may require explicit link migration.

Tree-sitter supplies declaration ranges and comment ownership, never symbol
identity. Its query captures declarations with `@symbol` and `@name`, lexical
containers with `@scope` and `@name`, and comments with `@comment`. Optional
`@modifier` and `@wrapper` captures support attributes, decorators and exports.
The owning declaration, determined by the attachment rules above, must have a
name span matching exactly one global SCIP identity. Otherwise the marker is
unsupported. File-owned markers need no symbol. Invalid packs and
duplicate extension assignments are diagnosed. Asset paths stay in the project.
With no grammar, SCIP symbol links still work, using the indexer's enclosing
range for content, or its definition token if no enclosing range is supplied.
Multiple declaration ranges of one identity are read as their enclosing source
interval. No explicit separator is configured.

The repository contains example runtime grammar/query assets built from
tree-sitter-rust 0.24.2, tree-sitter-python 0.25.0,
tree-sitter-javascript 0.25.0 and tree-sitter-typescript 0.23.2, with MIT notices.
They do not supply semantic identities without a configured indexer. Build other
grammars with `tree-sitter build --wasm` and supply a matching capture query.

## Migration and verification

Projects with code bindings use project format 3. For existing native-selector
bindings, remove `separator`, install/configure indexers, and replace selectors with exact
file/indexer-scoped descriptors. Old selectors are never interpreted as aliases.
Unreleased configurations with separate `[[code.indexers]]` entries must move
their command and position encoding into the matching `[[code.languages]]` entry
and remove the separate indexer entries. Project format remains 3; reset development configurations marked 4 to 3.
Keep existing schema format 3, item MIDs, relation names and grammar/query assets.
Projects without code bindings may retain their existing project format.

Grouped-comment attachment changes earlier file/enclosing-owned markers in
qualifying groups to declaration-owned markers. Their derived edges and affected
navigation cursors change; authored source, link syntax and format versions do not.

Real-indexer probes cover scip-typescript 0.4.0, scip-clang 0.4.0 and
rust-analyzer 1.97.1. C++ overload descriptors remain distinct after body edits,
insertion and reordering; deletion or rename removes the old identity.
TypeScript overloads share one callable descriptor, including after package
version changes. Regression fixtures in `tests/fixtures/scip` preserve real
indexer output and exercise CLI/MCP navigation, marker/inverse agreement,
package version independence, absent-overload failure and indexer failure.

## Validation and navigation

Missing file or symbol, ambiguous selector, and unsupported file/selector or
comment attachment produce distinct stable error codes at the authored
occurrence. Invalid relation permission and missing/ambiguous item markers
remain relation/reference errors. Validation invokes the configured indexer commands as described above. Moving, renaming, or deleting a symbol re-derives marker-owned
backlinks from current source; an old item-authored selector fails validation
until edited. Equivalent marker and item-authored assertions deduplicate as
one edge, while retaining both locations.

`related` accepts an item reference or `code:` reference and returns direct
code/item neighbours, canonical edges, direction, occurrence count, and
navigable source locations. `get` accepts the returned code reference and
reads UTF-8 file or symbol source in bounded pages; a binary file-only target
still resolves as a relation endpoint but `get` reports that its content is
not readable as text. Relation inspection shows both marker and item-authored
occurrences. CLI and MCP expose the same
results and diagnostics. Item detail summaries expose an item-authored inverse
target as `code_reference`; this is the exact `code:` reference and has no
item MID or item summary. Code files are discovered locally under the project
root using the repository's ignore rules; references to ignored or unsupported
files still resolve as file-only targets when explicitly authored. With an item
as `source` and a declared inverse alias, `relation add` writes the inverse
metadata to that item, rejecting an edge already asserted by either an item
or code marker. `relation remove` removes only item-authored inverse
occurrences of that edge. A remaining code marker keeps the canonical edge
present; mutation results report its remaining occurrence count and
`edge_exists: true`. An item-side remove with no item-authored occurrence fails
without changing code. A code endpoint as mutation `source` is rejected with
an explicit diagnostic: relation commands never modify code source files.
Removing an item-authored inline code relation leaves its `code:` target as
plain text in the item body.
Code markers are added or removed by directly editing their source comments.
:::

:::mara decision ADR-SCIP-CODE-IDENTITY
:mid: 01M3H5VS7R5JY96P7FH2JQBX2E
:title: Delegate symbol identity to pluggable SCIP indexers
:justifies: DES-CODE-TRACEABILITY

Use project-configured external SCIP indexers for semantic identity and optional
runtime Tree-sitter assets for source comment attachment. This keeps language
integrations out of the Mara binary and delegates overload disambiguation to
language maintainers. Scope descriptors by project-relative file and configured
language name; omit package metadata so ordinary version bumps preserve links.
Reject collisions and unresolved descriptors without name or position fallback.
Renames and moves may break links. Remove native selector separators because
syntax grammars do not define cross-language semantic identity.

Keep the indexer command and optional grammar assets in one language entry:
they describe one integration, and separate name lists duplicate configuration
and permit drift. Keep project format 3 during this unreleased alpha instead of
requiring migration between development-only format numbers.

Preserve literal SCIP backticks because Mara can parse them unambiguously and
readable descriptors help authors inspect exact targets. Escape only characters
that conflict with Mara's reference syntax; do not shorten or normalize semantic
descriptors for appearance, which could collapse distinct identities.
:::
