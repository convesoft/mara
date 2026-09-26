# Code traceability

Baseline contract retained for incremental restoration. The staged scanner
verification covers discovery only; graph, navigation, mutation and transport
obligations remain pending their capability checkpoints.

:::mara requirement REQ-CODE-TRACEABILITY
:mid: 01M3EQAFT3DPZ2VF1GWZBX2T6Y
:title: Resolve code associations as typed relations
:status: accepted
:kind: functional
:derives_from: SCN-TRACE-IMPLEMENTATION-AND-CHECKS

A project can declare code-to-item relations, navigate the resulting source
associations, and mutate item-authored inverse assertions through relation
commands. Source code comments remain directly authored in code. Missing,
ambiguous and unsupported code targets produce source-located diagnostics. A
source association does not establish passing test evidence.
:::

:::mara design DES-CODE-TRACEABILITY
:mid: 01M3EQAJBYC0Q2TAVD3KM6E11H
:title: Resolve code endpoints through language adapters
:status: accepted
:kind: interface
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
file path plus the adapter's exact symbol selector, or just the path for a
file-only endpoint. Identical assertions from either side count as one edge
with distinct source occurrences. Code links are structural associations;
`verifies` identifies a check definition and never claims a passing execution.
Incoming cardinality counts distinct canonical code-to-item edges, including
marker and item-authored assertions, at each eligible item. It does not count
multiple occurrences of one edge twice.

## Authored forms

An item may assert the inverse in metadata or a typed inline reference:

```markdown
:implemented_by_code: code:src/graph_constraints.rs::evaluate

Implemented by [[implemented_by_code:code:src/graph_constraints.rs::evaluate]].
```

The target grammar is `code:<project-relative-path>[::<language-native-symbol-selector>]`.
Split at the first `::` after `code:`; Rust selectors may contain further
`::`. Paths use `/`, contain only ordinary relative components, and cannot
escape the project through `..` or a symlink. Path and selector matching is
case-sensitive. A file-only target resolves to an existing regular file even
without a language adapter. A symbol target requires an adapter for that file.
No absolute path, authored byte span, line number, or generated identity is
part of the target spelling.

A source comment marker is `@mara <canonical-relation> <item-ID-or-MID>` on
its own comment line. Tree-sitter identifies comment nodes and source spans;
one shared parser searches only their text for markers. It does not scan raw
source as text. The relation must declare `code_source: true` and allow
the target item's flavour. Rust `//`, `///`, and block comments; Python `#`;
and JavaScript/TypeScript `//` and block comments are supported. A marker
immediately preceding a named declaration, separated only by whitespace and
declaration modifiers, attaches to that declaration. Otherwise a marker
inside a declaration body attaches to the deepest containing named declaration;
a top-level marker with no attached declaration attaches to the file. A marker
whose ownership cannot be determined is unsupported. Comment placement and
symbol selection are adapter responsibilities, not generic text heuristics.

## Adapter and selector boundary

Mara owns marker parsing of comment text, schema and item resolution, canonical edge identity,
diagnostics, navigation, and backlinks. A project supplies Tree-sitter language
bindings in `.mara/project.toml` format 3. Each entry maps extensions to a
project-relative WebAssembly grammar, a Tree-sitter query file, and the native
selector separator. The query captures declarations with `@symbol` and their
`@name`, lexical containers that are not direct targets with `@scope` and
`@name`, and comments with `@comment`. Optional `@modifier` captures mark
modifiers that appear as preceding siblings or leading children of a declaration;
optional `@wrapper` captures mark parent nodes containing a declaration and its
modifiers. Markers may attach before a captured modifier or wrapper, or between
a modifier and its declaration. Each pack describes its own attributes,
decorators, and exports through these captures. Mara loads and checks these assets
locally at runtime, uses the grammar and query to enumerate symbols and
comments, and searches only captured comment text for markers. The deepest
enclosing declaration body or immediately following declaration owns a marker.
Invalid packs and duplicate extension assignments are diagnosed. Normal corpus
operations fail when code indexing reports a problem, rather than returning a
partial relation graph; validation reports the problem as a diagnostic. An absent
pack leaves file-only endpoints usable. Adding another language or extension
does not require a Mara rebuild. Language packs do not change the shared
relation semantics or execute project code.

Rust and JavaScript adapters are configured for this repository. Python and
TypeScript packs are isolated test fixtures. Packs are project assets, not
language dependencies compiled into Mara; a project may configure other
extensions, including JSX and TSX, with a suitable grammar and query. A grammar can be built with the Tree-sitter
CLI's `tree-sitter build --wasm`; the generated file and query are supplied by
the project. Mara does not fetch or compile grammars during validation.
For example, one `.mara/project.toml` entry is:

```toml
format_version = 3
[[code.languages]]
name = "rust"
extensions = ["rs"]
grammar = ".mara/code/rust.wasm"
query = ".mara/code/rust.scm"
separator = "::"
```

The query uses standard Tree-sitter capture syntax, for example
`(function_item name: (_) @name) @symbol` and
`(line_comment) @comment`. Asset paths must remain inside the project.
The bundled grammar assets were built from tree-sitter-rust 0.24.2,
tree-sitter-python 0.25.0, tree-sitter-javascript 0.25.0, and
tree-sitter-typescript 0.23.2; their MIT notices accompany the files.

Rust selectors use `::` qualification through named modules, types, traits,
functions, and methods; implementation blocks supply their type as a lexical
scope. Python and JavaScript/TypeScript selectors use `.`
qualification through named classes, functions, and methods. Nested named
functions use their lexical owners. Named declarations may be selected
directly; computed names, anonymous constructs, macro-expanded declarations,
and overloaded signatures without a unique native selector are unsupported.
One selector must resolve to exactly one declaration. No adapter chooses the
first of multiple matches.

The scanner's `CodeIndex::load` library boundary returns a disposable file
projection and explicit loading/parsing problems. Files preserve source bytes,
selectors, marker occurrences and source spans; accepted adapter asset paths
remain available for read-operation snapshot identity. No code is executed or
modified. This dependency does not resolve item targets or expose a CLI/MCP
operation by itself. Full corpus reads compose document and code discovery and
reject reported code problems before query operations.

## Validation and navigation

Missing file or symbol, ambiguous selector, and unsupported file/selector or
comment attachment produce distinct stable error codes at the authored
occurrence. Invalid relation permission and missing/ambiguous item markers
remain relation/reference errors. Validation does not access the network or
execute code. Moving, renaming, or deleting a symbol re-derives marker-owned
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

:::mara verification VER-CODE-DISCOVERY
:mid: 01M3FZXJ1PRPJPV1CE5YT1W304
:title: Check local adapter loading and code discovery
:status: accepted
:method: test
:level: integration
:verifies: REQ-CODE-TRACEABILITY
:verifies: DES-CODE-TRACEABILITY

Run `cargo test --locked --test code_discovery` with real supplied Wasm grammars in disposable projects with their own Git/configuration state. Copy only required adapters into each fixture; Python/TypeScript test assets do not expand the repository's configured scanner.

Check native selectors, nested and modifier/wrapper ownership, content spans, comment-only marker parsing and unsupported attachment. Check deterministic discovered files, ignore behavior and independence from document filters; malformed/unavailable/confined assets and required capture pairing; unreadable sources, invalid marker diagnostics, and internal versus external file symlinks. Loading preserves source bytes and exposes explicit problems alongside independently available files; invalid packs prevent a partial scan.

This method verifies the code-discovery dependency through `CodeIndex::load`, its file projection and asset inventory. It does not establish exact endpoint resolution, semantic relation permission/target validation, navigation, mutation, cursor continuation or CLI/MCP code operations. Existing CLI/MCP tests run as regressions; later item-list verification must prove full corpus composition and code-sensitive continuations.
:::
