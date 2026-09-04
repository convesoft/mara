# Mara document format

:::mara design DES-DOCUMENT-FORMAT
:mid: 01M1PXP2KG381MM1VNN6XC7S4M
:title: Mara document format

This contract defines the smallest authoring surface needed to dogfood Mara.

## Documents

- A Mara document is a UTF-8 Markdown file named `*.mara.md`.
- Markdown outside Mara item blocks remains ordinary document content.
- Mara syntax inside fenced or inline code is example text, not project data.

## Items

An item has an explicit namespace, flavour, and complete human-readable ID:

```markdown
:::mara requirement REQ-FAIL-SAFETY
:title: Fail safely and observably
:depends_on: REQ-ACTIONABLE-DIAGNOSTICS

Failures preserve user data and produce actionable diagnostics.
:::
```

- The opening line is `:::mara <flavour> <id>` at the start of a line, with no
  other tokens.
- A flavour matches `[a-z][a-z0-9]*(?:_[a-z0-9]+)*`.
- An ID matches `[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+`, is mandatory and unique in
  the repository, and includes its human-meaningful prefix: `REQ-1`, not `1`.
- Flavours and ID prefixes follow the project taxonomy.
- IDs are stable handles. Renaming one requires updating all of its references.
- An item closes with an exact standalone `:::`. Items cannot nest.

## Metadata and body

- Metadata is the contiguous sequence of `:key: value` lines after the opener.
- Keys use lowercase snake case. Values are single-line scalars whose
  surrounding whitespace is not semantic.
- Every item has exactly one non-empty `title` entry.
- Repeated keys are preserved in source order.
- The first blank line begins the Markdown body. Metadata-shaped lines after
  that boundary remain body content.
- The body may be empty.

## Machine identity

- `mid` is reserved structural metadata, not a configurable field.
- Every item has exactly one Mara-generated MID on the line immediately after
  its opener.
- A MID is a raw canonical uppercase 26-character ULID with no prefix.
- The MID is repository-wide unique and immutable for the item's lifetime.
- Callers never provide, copy, edit, or update MIDs through item operations.
- Do not create or copy placeholder MIDs by hand. Existing pre-alpha items
  receive MIDs through one deliberate backfill before identity-dependent
  editing is used.

## References and relations

- `[[REQ-FAIL-SAFETY]]` is a readable internal mention resolved by exact ID.
- Typed relations are metadata entries whose key names the relation and whose
  value is one target handle. Repeat the key for multiple targets.
- Use only relation names whose meaning is defined by the project corpus.
- Relations resolve target handles to MIDs internally once machine identity
  exists. Inverses and backlinks are derived and are not authored a second time.
- Inline references are mentions; typed relations are authored in metadata.

## In-memory projection

- Mara discovers project-relative `*.mara.md` files through the configured
  content include patterns and reads those canonical files directly.
- Documents are ordered by project-relative path and items remain in source
  order so repeated loads of unchanged files produce the same model.
- Each item retains its ordered metadata, exact Markdown body, schema-defined
  typed relations, body mentions outside fenced and inline code, and source
  locations.
- A source location contains the project-relative path, an end-exclusive UTF-8
  byte span, and one-based start and end lines.
- The in-memory model is a disposable projection, never an authoring authority.

## Evolution

The document syntax has no embedded schema or project configuration. Introduce
validation or new syntax only for a demonstrated workflow. Persisted format
versions remain independent from the Mara application version.
:::

:::mara decision ADR-RUSHDOWN-PARSER-ADAPTER
:mid: 01M1PXP2KGG86FFPSNS8QWEXRZ
:title: Use Rushdown behind a Mara-owned Markdown adapter
:justifies: DES-DOCUMENT-FORMAT

Use Rushdown custom block and inline extensions to recognize Mara structures
with Markdown-aware code and raw-context handling and exact source spans. A
private adapter converts the Rushdown result immediately into Mara-owned values,
containing third-party AST and parser API churn behind that boundary.

This does not add the explicitly deferred complete Markdown AST to the alpha
contract.
:::
