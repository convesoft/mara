# Mara wire contracts v1

This reference groups the typed machine-interface design contracts for Mara
v0.1. Each contract below is independently addressable in the Mara graph.

:::design m_01KY82WX4VM6TY9W0AADC1VJHP
:id: DES-WIRE-SOURCE-SPANS
:title: Source position and span wire representation
:status: accepted
:kind: interface
:satisfies: REQ-LANGUAGE-SOURCE-SPANS

## Source positions and spans

All offsets address the original UTF-8 file bytes before newline or escape
normalization. A `SourceSpan` has this exact JSON shape and key order:

```json
{
  "path": "docs/example.mara.md",
  "start_byte": 0,
  "end_byte": 12,
  "start_line": 1,
  "start_column": 1,
  "end_line": 1,
  "end_column": 13
}
```

- `path` is a `/`-separated, project-relative, normalized UTF-8 path. It never
  starts with `/`, contains `.` or `..` segments, or exposes a host path.
- Byte offsets are zero-based unsigned integers. `start_byte` is inclusive and
  `end_byte` is exclusive. The document span is `[0, file_length)`.
- Lines and columns are one-based. End line and column identify the exclusive
  end position corresponding to `end_byte`.
- A column counts Unicode scalar values since the beginning of its line plus one;
  a tab counts as one scalar and display width is irrelevant.
- LF is one line terminator byte. CRLF is one logical terminator with two bytes.
  The next byte after either terminator is column one of the next line.
- Empty spans are valid when start equals end. Spans shall not split a UTF-8 code
  point or the two-byte CRLF sequence.

Source edits use bytes as authority. Human diagnostics render the one-based line
and column. Every optional span in the JSON contracts below is represented by
explicit null rather than omission.

:::

:::design m_01KY82WX4WQK4QD88SQMD9MBDH
:id: DES-WIRE-DIAGNOSTICS
:title: Diagnostic wire model, catalogue, and ordering
:status: accepted
:kind: interface
:satisfies: REQ-DIAGNOSTIC-MODEL
:satisfies: REQ-DIAGNOSTIC-ORDER
:satisfies: REQ-DIAGNOSTIC-OUTPUT
:satisfies: REQ-REFERENCE-FAILURES

## Diagnostic model

A diagnostic has these keys in this order:

```json
{
  "code": "identity.duplicate_mid",
  "severity": "error",
  "message": "machine identity is used by more than one item",
  "primary": null,
  "related": [],
  "item": null,
  "context": {
    "field": null,
    "relation": null,
    "target": null
  },
  "details": {}
}
```

`severity` is exactly `error`, `warning`, or `info`. `related` entries have keys
`message` then `span`; `span` is never null. `item` is either null or an object
with `mid` then `id`, where `id` may be null. Context values are strings or null.
`details` is a JSON object whose code-specific keys are sorted by UTF-8 byte
order. Messages may improve without a format-version change; codes, severities,
locations, context, and details carry machine meaning.

Diagnostics sort by: presence of primary path, path UTF-8 bytes, `start_byte`,
severity rank (`error`, `warning`, `info`), code, then canonical JSON bytes of
`details`. A missing primary span sorts after every present span.

### v0.1 diagnostic-code catalogue

The following code set is closed for format version 1. More specific details are
placed in `details`; implementations shall not synthesize ad-hoc code strings.

| Code | Default severity | Meaning |
|---|---|---|
| `project.not_found` | error | Upward discovery found no `.mara/project.toml`. |
| `project.path_outside_root` | error | A configured path escapes the project root. |
| `project.symlink_rejected` | error | A discovered path violates symlink policy. |
| `project.duplicate_file` | error | More than one discovery path resolves to one file identity. |
| `config.io` | error | Project configuration could not be read. |
| `config.syntax` | error | TOML syntax is invalid. |
| `config.duplicate_key` | error | TOML declares a key more than once. |
| `config.unknown_key` | error | Configuration contains a key outside format v1. |
| `config.invalid_value` | error | A configuration value has invalid type or value. |
| `schema.io` | error | The schema file could not be read. |
| `schema.syntax` | error | YAML syntax or the allowed YAML profile is invalid. |
| `schema.duplicate_key` | error | A YAML mapping key is duplicated. |
| `schema.unsupported_format` | error | `format_version` is not supported. |
| `schema.unknown_key` | error | A mapping contains a key outside its v1 declaration. |
| `schema.invalid_name` | error | A schema-defined name violates its name grammar. |
| `schema.invalid_pattern` | error | A declared regular expression is invalid. |
| `schema.invalid_declaration` | error | A declaration violates a cross-key or reference invariant. |
| `content.io` | error | A selected content file could not be read. |
| `content.invalid_utf8` | error | A content file is not valid UTF-8. |
| `syntax.invalid_item_header` | error | A Mara opening line is malformed. |
| `syntax.invalid_metadata` | error | Metadata syntax is malformed before the body boundary. |
| `syntax.unclosed_item` | error | An opened item has no exact closing marker. |
| `syntax.invalid_inline_reference` | error | Inline reference syntax or context is invalid. |
| `identity.invalid_mid` | error | MID syntax or canonical ULID value is invalid. |
| `identity.duplicate_mid` | error | More than one live item has one MID. |
| `identity.invalid_display_id` | error | A display ID violates its flavour declaration. |
| `identity.duplicate_display_id` | error | More than one live item has one display ID. |
| `item.unknown_flavour` | error | An item flavour is absent from the schema. |
| `item.missing_value` | error | A required built-in or field is absent or empty. |
| `item.unknown_key` | error | Metadata is neither a built-in, field, nor authorable relation. |
| `field.invalid_scalar` | error | Text cannot convert to the declared scalar type. |
| `field.invalid_enum` | error | A value is outside the declared enum. |
| `field.pattern_mismatch` | error | A string or display ID fails its full-match pattern. |
| `field.repetition` | error | A non-repeatable field occurs more than once. |
| `reference.unresolved` | error | An internal reference resolves to no active MID or display ID. |
| `reference.ambiguous` | error | An internal reference matches more than one live item. |
| `reference.external_scheme` | error | An external target scheme is not permitted. |
| `relation.unknown` | error | A typed relation name is not authorable in this context. |
| `relation.invalid_source` | error | Canonical source flavour or derived kind is not permitted. |
| `relation.invalid_target` | error | Canonical target flavour or external kind is not permitted. |
| `relation.self_reference` | error | A relation forbids its source-target self-edge. |
| `relation.cardinality` | error | Normalized edge count violates declared bounds. |
| `relation.cycle` | error | An acyclic relation contains a cycle. |
| `relation.duplicate_occurrence` | warning | An exact authored occurrence duplicates an existing edge occurrence. |
| `rule.failed` | configured | A project rule failed; `details.rule` contains its schema name. |
| `rule.skipped` | info | A rule could not run because a prerequisite model was unavailable. |
| `edit.dirty_worktree` | error | Clean-write policy rejected the operation. |
| `edit.stale_source` | error | A planned edit no longer matches its parsed preimage. |
| `transaction.incomplete` | error | A prior transaction requires explicit recovery. |
| `transaction.recovery_failed` | error | Recovery could not prove restoration or completion. |

For `reference.ambiguous`, `primary` is the exact authored reference occurrence
and `details` has exactly these keys:

```json
{
  "candidate_mids": [
    "m_01KY0000000000000000000001",
    "m_01KY0000000000000000000002"
  ],
  "reference": "REQ-DUPLICATE"
}
```

`reference` preserves the target token exactly as authored, excluding inline
delimiters, an optional relation qualifier, and an optional label.
`candidate_mids` contains every distinct live candidate MID in ascending UTF-8
byte order and therefore contains at least two entries.
`related` contains one entry per candidate in that same order; its message is
`candidate: <MID>` and its span is the candidate item's opening line.

For `rule.failed`, the schema-selected severity replaces the catalogue default.
An implementation that needs a new semantic category must introduce a later wire
format version or first add the code to this reference.

:::

:::design m_01KY82WX4X96E7YBD6HYMEVKDG
:id: DES-WIRE-COMMAND-OUTPUT
:title: Command status, JSON envelope, and query contracts
:status: accepted
:kind: interface
:satisfies: REQ-VALIDATION-EXIT-CODES
:satisfies: REQ-QUERY-FORMATS

## Process exit status

All v0.1 commands use this complete table:

| Exit | JSON status | Meaning |
|---:|---|---|
| `0` | `ok` | The requested operation completed and no diagnostic failed policy. |
| `1` | `invalid` | Input was processed and one or more diagnostics fail configured policy. |
| `2` | `failed` | Invalid invocation, discovery/read/write/Git failure, or unavailable prerequisite prevented the requested result. |

Schema and content defects that can be represented as diagnostics use exit `1`.
Missing project files, unreadable required files, invalid CLI arguments, failed
write preconditions, and transaction failures use exit `2`. Warnings produce
exit `1` only when warning escalation is enabled. No other exit code is part of
the v0.1 contract; termination by the operating system remains outside it.

## JSON command envelope

`check`, `schema check`, `list`, `show`, `trace`, and `index` shall accept
`--format json`. They emit exactly one envelope to stdout and no human prose to
stderr for every handled result:

```json
{
  "format": "mara.command",
  "version": 1,
  "command": "check",
  "status": "ok",
  "project": null,
  "diagnostics": [],
  "data": null,
  "error": null
}
```

`command` is one of `check`, `schema_check`, `list`, `show`, `trace`, or `index`.
`project` is null until project identity is known; otherwise it contains keys
`name`, `root`, `schema_name`, `schema_version`, and `schema_path`. `root` is
always `.` and both paths are normalized project-relative strings.

`error` is null for `ok` and `invalid`. For `failed`, it is an object with keys
`code`, `message`, and `details`; `data` is null. Operational error codes are
`cli.invalid_arguments`, `project.unavailable`, `io.failed`, `git.precondition`,
`transaction.incomplete`, `transaction.failed`, and `internal.failed`. Details
keys are sorted; internal causes must not expose secrets or absolute host paths.

The status-to-payload contract is exact: `ok` has the command-specific `data`
below and null `error`; `invalid` has null `data` and null `error`, with all
policy-failing validation results represented by `diagnostics`; `failed` has
null `data` and a non-null operational `error`. No command emits a partial
success payload for an `invalid` result.

Successful command data has these exact forms:

- `check` and `schema_check`: `{"summary": Summary}`;
- `list`: `{"filters": ListFilters, "items": [ItemSummary]}`;
- `show`: `{"item": Item}`;
- `trace`: `{"focus_mid": string, "direction": string, "max_depth": integer,
  "nodes": [TraceNode], "paths": [TracePath]}`;
- `index`: `{"path": string, "sha256": string, "summary": Summary}`.

`Summary` keys are `documents`, `items`, `source_nodes`, `edges`, `mentions`,
`external_nodes`, `errors`, `warnings`, and `info`, all non-negative integers.
`ItemSummary` keys are `mid`, `id`, `flavour`, `title`, and `source`; `mid` and
`flavour` are strings, `id` and `title` are strings or null, and `source` is a
non-null `SourceSpan`.

A graph endpoint is represented by the exact `NodeRef` tagged union:

- item: `{"kind":"item","mid":string}`;
- derived source: `{"kind":"source_span","source":SourceSpan,"symbol":string|null}`;
- external object: `{"kind":"external","uri":string}`.

Only item and source-span references may be canonical edge sources. Only item and
external references may be canonical edge targets. A source-span node's identity
is the tuple of source path, start byte, end byte, and symbol; line and column
values must correspond to those bytes but do not create a different identity.

A `TraceNode` is the corresponding exact tagged union with query details:

- item: `{"kind":"item","item":ItemSummary}`;
- derived source: `{"kind":"source_span","source":SourceSpan,"symbol":string|null}`;
- external object: `{"kind":"external","uri":string,"scheme":string}`.

`TracePath` keys are `nodes` (ordered `NodeRef` sequence) and `edges` (ordered
`TraceStep` sequence). A `TraceStep` has `relation`, `traversal`, `source`, and
`target`. `relation` and `traversal` are strings; `source` and `target` are the
canonical `NodeRef` endpoints. `traversal` is `outgoing` when the path follows
source to target or `incoming` when it moves from target to source.

Trace returns every distinct simple canonical-edge path beginning at the focus
and containing from one through `max_depth` steps allowed by the selected
direction. A simple path never repeats a `NodeRef` identity, so cycles terminate.
The zero-step focus path is not returned. Authored occurrences do not create
additional paths.
Canonical edges with the same endpoints but different relation names create
distinct paths; otherwise paths are deduplicated by their ordered `TraceStep`
sequence including traversal. The `nodes` collection is the deduplicated union
of the focus and all returned path nodes. It sorts by node kind and then item MID,
source-span identity tuple, or external URI. Paths sort by step count, canonical
JSON bytes of their node-reference sequence, then canonical JSON bytes of their
steps.

`mara list` accepts repeatable `--flavour <snake_name>` and repeatable
`--field <snake_name>=<raw-value>` options. A field option splits at its first
`=`; the value may be empty. Unknown flavours are invalid arguments. Candidate
flavours are the explicit flavour set, or every schema flavour when none is
given. An unknown field on every candidate flavour is invalid.

Each raw field value is ASCII-trimmed and independently converted against the
field declaration of every candidate flavour that defines that name, using the
schema scalar rules including enum and pattern constraints. A failed conversion
excludes that value only for that flavour; the command is invalid when a raw
value converts for no candidate flavour. A candidate flavour that defines none
of the successfully compiled values for a requested field cannot match.

`ListFilters` has `flavours` then `fields`. `flavours` is the UTF-8-byte-sorted,
deduplicated explicit candidate sequence, or an empty sequence when all flavours
are candidates. `fields` sorts by field name. Each field filter has `name`,
`raw_values`, and `compiled`; normalized raw values are sorted and deduplicated.
`compiled` sorts by flavour and contains objects with `flavour`, `type`, and
`values`; typed values sort by canonical JSON bytes and are deduplicated. Values
within one field or the flavour option combine with OR. Different field names
and the flavour constraint combine with AND.

An item matches only when its flavour satisfies the flavour constraint and, for
every requested field name, the item has at least one authored typed value equal
to at least one compiled value for that item's flavour. Thus repeatable fields use
existential matching; an absent field or a field without compiled values for the
item's flavour does not match. Equality is exact UTF-8 byte equality for `string`
and `enum`, exact value equality for `integer` and `boolean`, and finite IEEE-754
numeric equality for `number`, with negative zero equal to positive zero. Values
of different declared types are never compared. Duplicate authored values do not
change the predicate or result ordering.

`mara mid` performs normal upward project discovery and loads the configured
schema identity, but does not discover or parse content. Outside a valid Mara
project it exits `2`. On success its human interface writes exactly one MID with
the configured prefix followed by LF to stdout. Mutating commands in v0.1 have
review-oriented human output only; that wording is not a compatibility surface.
Their exit status still follows the table above.

:::

:::design m_01KY82WX4Y5ATY4W0WN3W76FA0
:id: DES-WIRE-INDEX-PROJECTION
:title: Canonical JSON and generated index representation
:status: accepted
:kind: data_model
:satisfies: REQ-INDEX-CONTENT
:satisfies: REQ-INDEX-DETERMINISTIC
:satisfies: REQ-INDEX-GIT-PROVENANCE

## Canonical JSON encoding

Command JSON and index JSON use UTF-8, two-space indentation, LF line endings,
no trailing spaces, and exactly one final LF. Keys appear in the order defined
by this document; dynamic object keys use ascending UTF-8 byte order. Arrays use
the ordering specified for their type. Strings use JSON escapes only for quote,
backslash, and control characters; non-ASCII Unicode is emitted literally.
Numbers are finite JSON numbers with shortest round-trippable representation.
Optional fields are always present with null; they are never omitted. SHA-256
digests are lowercase 64-character hexadecimal strings.

## Index document

The configured index is a canonical JSON object with these keys in order:

```json
{
  "format": "mara.index",
  "version": 1,
  "project": {},
  "git": {},
  "documents": [],
  "items": [],
  "source_nodes": [],
  "edges": [],
  "mentions": [],
  "external_nodes": [],
  "diagnostics": []
}
```

It is written only when the normalized project model exists and no diagnostic
fails policy. It may contain non-failing warning and info diagnostics. It never
contains the generation time, absolute paths, random values, or database-local
identifiers.

The index writer serializes the complete canonical document to an exclusively
created sibling temporary file, flushes that file, atomically replaces the
configured index in the same directory, and flushes the parent directory. A
failure before replacement leaves any previous index untouched. A failure after
replacement but before the directory flush reports an operational failure; after
a crash, the destination may contain the complete previous or complete new index,
never a partial document. The writer uses a uniquely named sibling temporary file
created with no-follow and exclusive-create semantics. It removes that file on a
handled pre-replacement failure; an abandoned temporary file is ignored as
non-authoritative state and is never parsed as the configured index.

### Project and Git objects

`project` keys are `name`, `schema`, and `content`. `schema` has `name`,
`version`, `format_version`, `path`, and `sha256`. `content` has `include` and
`exclude`, preserving configured sequence order.

`git` keys are `available`, `commit`, `branch`, `project_path`, and `dirty`.
When unavailable, `available` is false and all other values are null. Otherwise,
`commit` is the full object ID, detached HEAD makes `branch` null,
`project_path` is repository-relative with `/` separators, and `dirty` is true
when any selected config, schema, or content path differs from HEAD or is
untracked. Ignored derived files do not make it dirty.

### Document hierarchy

Documents sort by normalized path. A document has keys:

`path`, `sha256`, `line_ending`, `span`, `preamble`, `sections`, and `item_mids`.

`line_ending` is `lf`, `crlf`, `mixed`, or `none`. `preamble` is the ordered
narrative and item placements before the first heading. `sections` is the
ordered top-level section sequence. `item_mids` lists every contained item in
source order.

A section has keys `level`, `title`, `span`, `heading_span`, `content`, and
`children`. `content` contains placements directly under that heading before a
child heading. Children are source ordered and structurally nested by Markdown
heading level.

A placement has `kind`, then exactly one payload:

- narrative: `{"kind":"narrative","block": NarrativeBlock}`;
- item: `{"kind":"item","mid":string,"span":SourceSpan}`.

`NarrativeBlock` keys are `kind`, `markdown`, `span`, and `mentions`. `kind` is
`paragraph`, `heading`, `list`, `quote`, `code`, `table`, `thematic_break`,
`html`, or `other`. `markdown` is the exact source substring. A complex Markdown
container may contain smaller Mara-owned nodes in memory, but the v1 index uses
the exact block Markdown as its lossless rendering contract. `mentions` embeds
the complete mention objects whose source spans fall within the block.

### Item objects

Global items sort by MID. An `Item` has keys in this order:

`mid`, `id`, `flavour`, `title`, `body_markdown`, `document`, `source`,
`header_source`, `body_source`, `metadata`, `fields`, `outgoing`, `incoming`, and
`mentions`.

`id` and `title` are strings or null. `document` is a normalized path.
`mid` and `flavour` are strings. `body_markdown` is the exact source substring of
the item body. `source` and `header_source` are non-null `SourceSpan` values:
`source` covers the complete opening-through-closing item block and
`header_source` covers the opening line without its line terminator.
`body_source` is a non-null `SourceSpan` covering exactly `body_markdown`; an
empty body uses an empty span at the body insertion position.
`metadata` preserves authored order and contains entries with keys `key`,
`raw_value`, and `source`; both values are strings and `source` is the non-null
span of the complete metadata entry without its line terminator. `fields` sorts
by field name and contains entries with `name`, `type`, and `values`; `name` and
`type` are strings, and each value has `value` in its declared JSON scalar type
and a non-null `source` span equal to its metadata entry source. Repeated values
retain source order.

`outgoing` and `incoming` embed the complete canonical edge objects defined
below, including inverse name and every occurrence. Both arrays use canonical
edge ordering. Incoming retains canonical source-to-target direction and is a
presentation view, not another stored edge. `mentions` embeds complete mention
objects in source order. This makes the same `Item` representation self-contained
in `show` output; the index's global edge and mention arrays are flattened query
collections, not object-identity tables.

### Derived source nodes, edges, mentions, and external nodes

A `SourceNode` has keys `source` and `symbol`. `source` is a non-null `SourceSpan`
inside a project-contained source file supplied by a derived adapter and `symbol`
is a string or null. Source nodes sort and deduplicate by the source-span identity
tuple defined for `NodeRef`. They are derived projection nodes, not authored Mara
items, and never receive a MID.

Canonical edges sort by canonical JSON bytes of source `NodeRef`, relation UTF-8
bytes, then canonical JSON bytes of target `NodeRef`. An edge has keys `source`,
`relation`, `inverse_name`, `target`, and `occurrences`. `source` is an item or
source-span `NodeRef`; `target` is an item or external `NodeRef`; `relation` is a
string and `inverse_name` is a string or null. Occurrences sort by source path and
start byte and contain `origin`, `authoring_name`, and `source`. `origin` and
`authoring_name` are strings and occurrence `source` is a non-null `SourceSpan`.
Origin is `canonical_metadata`, `inverse_metadata`, `typed_inline`, or
`derived_source`. Repeated occurrences do not duplicate the canonical edge.

An item's `outgoing` contains edges whose source is that item's `NodeRef`. Its
`incoming` contains edges whose target is that item's `NodeRef`, including edges
from derived source nodes. A source-node edge is therefore globally representable
and appears as an incoming backlink without inventing a source MID.

A mention has keys `document`, `source_item_mid`, `target`, `label`, and `source`.
`document` is a normalized path, `source_item_mid` and `label` are strings or
null, `target` is an item or external `NodeRef`, and `source` is a non-null
`SourceSpan`; source-span mention targets are forbidden. Mentions sort by
document, start byte, target kind, and target identity. Embedded item and
narrative mentions use this same complete shape. An external node has keys `uri`
and `scheme`; both are strings, nodes sort by URI bytes, and nodes are
deduplicated by exact URI.

Diagnostics use the model and ordering above. Because the index is derived, a
consumer shall verify `format`, `version`, project identity, schema digest, and
Git provenance before using it as context for a selected repository state.
:::
