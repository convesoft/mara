# Code traceability

Accepted contract for MARA-71. The [Rust evaluation](traceability.mara.md#code-traceability-pilot)
is evidence for this design; this document owns production semantics.

:::mara requirement REQ-CODE-TRACEABILITY
:mid: 01M3EQAFT3DPZ2VF1GWZBX2T6Y
:title: Resolve code associations as typed relations

A project can declare code-to-item relations and navigate the resulting source associations. Missing, ambiguous and unsupported code targets produce source-located diagnostics. A source association does not establish passing test evidence.
:::

:::mara design DES-CODE-TRACEABILITY
:mid: 01M3EQAJBYC0Q2TAVD3KM6E11H
:title: Resolve code endpoints through language adapters
:satisfies: REQ-CODE-TRACEABILITY

Schema format 3 accepts optional `code_source: true` on a directed relation.
Such a declaration has `source: []`, a nonempty item-flavour `target`, and an
optional `inverse` for authoring from the item. It cannot be symmetric,
same-flavour, external, or code-to-code. A relation without `code_source`
retains its current item/external behaviour. For example:

```yaml
relations:
  implements:
    description: The code implements the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: implemented_by
  verifies:
    description: The code defines a check of the requirement.
    source: []
    target: [requirement]
    code_source: true
    inverse: verified_by
```

The canonical edge is `(relation, code endpoint, item MID)`. Code endpoints
have no MID or item flavour. Their identity is the exact project-relative
file path plus the adapter's exact symbol selector, or just the path for a
file-only endpoint. Identical assertions from either side count as one edge
with distinct source occurrences. Code links are structural associations;
`verifies` identifies a check definition and never claims a passing execution.

## Authored forms

An item may assert the inverse in metadata or a typed inline reference:

```markdown
:implemented_by: code:src/graph_constraints.rs::evaluate

Implemented by [[implemented_by:code:src/graph_constraints.rs::evaluate]].
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
its own comment line. The relation must declare `code_source: true` and allow
the target item's flavour. Rust `//`, `///`, and block comments; Python `#`;
and JavaScript/TypeScript `//` and block comments are supported. A marker
immediately preceding a named declaration, separated only by whitespace and
declaration modifiers, attaches to that declaration. Otherwise a marker
inside a declaration body attaches to the deepest containing named declaration;
a top-level marker with no attached declaration attaches to the file. A marker
whose ownership cannot be determined is unsupported. Comment placement and
symbol selection are adapter responsibilities, not generic text heuristics.

## Adapter and selector boundary

Mara owns marker parsing, schema and item resolution, canonical edge identity,
diagnostics, navigation, and backlinks. Each language adapter owns syntax
parsing, comment recognition and attachment, symbol enumeration, and selector
resolution. Adding a language must preserve the shared relation semantics.
The initial adapters are Rust, Python, JavaScript, and TypeScript. Extension
selects the adapter (`.rs`, `.py`, `.js`/`.mjs`/`.cjs`, `.ts`/`.mts`/`.cts`);
TypeScript `.tsx` and JavaScript `.jsx` require explicit adapter support
before use.

Rust selectors use `::` qualification through named modules, types, traits,
functions, and methods; a trait implementation method uses
`<Type as Trait>::method`. Python and JavaScript/TypeScript selectors use `.`
qualification through named classes, functions, and methods. Nested named
functions use their lexical owners. Named declarations may be selected
directly; computed names, anonymous constructs, macro-expanded declarations,
and overloaded signatures without a unique native selector are unsupported.
One selector must resolve to exactly one declaration. No adapter chooses the
first of multiple matches.

For the second-language proof, Python, JavaScript, and TypeScript fixtures
each connect a requirement to a named implementation method and a nested
verification function. They exercise the same canonical relation, marker,
inverse-authoring, navigation, and diagnostic paths as the Rust workflow.
The Rust workflow uses `REQ-RELATION-CARDINALITY`,
`src/graph_constraints.rs::evaluate`, and its CLI/MCP verification in
`tests/cli.rs`.

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
reads the current file or symbol source in bounded pages. Relation inspection
shows both marker and item-authored occurrences. CLI and MCP expose the same
results and diagnostics. Code files are discovered locally under the project
root using the repository's ignore rules; references to ignored or unsupported
files still resolve as file-only targets when explicitly authored.
:::
