# Mara schema format v1

This reference groups the typed design contracts for schema format version 1.
Each contract below is independently addressable in the Mara graph.

:::design m_01KY82WX4K6CV6S8RJJPAQ39Z8
:id: DES-SCHEMA-DOCUMENT-PROFILE
:title: Schema document profile, root mapping, and identity
:status: accepted
:kind: data_model
:satisfies: REQ-SCHEMA-VERSION
:satisfies: REQ-SCHEMA-STRICT
:satisfies: REQ-SCHEMA-IDENTITY
:satisfies: REQ-SCHEMA-NO-DEFAULTS
:satisfies: REQ-SCHEMA-SINGLE-FILE

## YAML profile

A schema is exactly one UTF-8 YAML 1.2 Core document. A loader shall:

- reject invalid UTF-8, multiple documents, duplicate mapping keys, custom tags,
  anchors, aliases, and merge keys;
- preserve source spans for every key and value;
- resolve only Core null, boolean, integer, and floating scalars automatically;
- require every key to be a string and reject null wherever this reference does
  not explicitly permit it;
- treat mappings as unordered semantic input and sequences as ordered input;
- reject an empty sequence or mapping where a declaration below says non-empty.

Comments have no semantic meaning. Empty optional top-level collections use `{}`
for a mapping and `[]` for a sequence; null is not an empty collection. Numeric
configuration values are base-ten YAML integers without a sign unless a section
explicitly says otherwise.

Names called `snake_name` shall match `[a-z][a-z0-9]*(?:_[a-z0-9]+)*`.
Names called `kebab_name` shall match `[a-z][a-z0-9]*(?:-[a-z0-9]+)*`.
External schemes shall match `[a-z][a-z0-9+.-]*`. All names are case-sensitive.

Patterns use the Rust `regex` crate syntax with Unicode enabled. Mara applies a
pattern to the entire semantic string, equivalent to wrapping it in `\A(?:...)\z`.
Look-around and backreferences are therefore unsupported. An invalid pattern is
a schema error at the pattern value.

## Closed top-level mapping

The root mapping has exactly these keys in v1:

| Key | Type | Required | Default |
|---|---|---:|---|
| `format_version` | integer | yes | none |
| `schema` | mapping | yes | none |
| `identity` | mapping | yes | none |
| `flavours` | mapping of flavour declarations | yes; may be empty | none |
| `relations` | mapping of relation declarations | no | `{}` |
| `rules` | sequence of rule declarations | no | `[]` |

`imports`, `extends`, defaults, scripts, plugins, environment interpolation, and
every other top-level key are errors in v1.

### Format and schema identity

`format_version` must be integer `1`.

The `schema` mapping contains exactly:

| Key | Type | Required |
|---|---|---:|
| `name` | `kebab_name` string | yes |
| `version` | semantic-version string | yes |

The version uses SemVer 2.0.0 core syntax, including optional prerelease and
build identifiers. Mara compares it as an opaque declared identity in v1; it
does not select behaviour from the project schema version.

### Machine identity representation

The `identity` mapping contains exactly one key, `mid`. The `mid` mapping contains
exactly:

| Key | Type | Required | v1 value |
|---|---|---:|---|
| `format` | string | yes | `ulid` |
| `prefix` | string | yes | `[a-z][a-z0-9]*_` |

An authored MID is the configured prefix followed by 26 uppercase canonical
Crockford Base32 ULID characters. The ULID alphabet excludes `I`, `L`, `O`, and
`U`; the 128-bit value must be within the canonical ULID range. Requiredness,
immutability, and project-wide uniqueness are platform invariants and are not
configurable schema keys.

:::

:::design m_01KY82WX4M0P3TW4KGNGCDCRD1
:id: DES-SCHEMA-FLAVOUR-DECLARATIONS
:title: Flavour, built-in, and field declaration model
:status: accepted
:kind: data_model
:satisfies: REQ-SCHEMA-FLAVOURS
:satisfies: REQ-SCHEMA-BUILTINS
:satisfies: REQ-SCHEMA-FIELDS
:satisfies: REQ-SCHEMA-REPEATED-FIELDS

## Flavour declarations

Each key of `flavours` is a unique `snake_name`. Its value contains only:

| Key | Type | Required | Default |
|---|---|---:|---|
| `label` | non-empty string | yes | none |
| `description` | non-empty Markdown string | yes | none |
| `guidance` | flavour guidance declaration | yes | none |
| `id` | built-in declaration | yes | none |
| `title` | built-in declaration | yes | none |
| `body` | built-in declaration | yes | none |
| `fields` | mapping of field declarations | no | `{}` |

`label` is the concise human-facing singular name shown by reference views and
interfaces. `description` defines the flavour's semantic purpose. The compiled
`FlavourDefinition` retains both strings and the structured guidance below with
their schema source spans; they are schema data, not item fields and not
hardcoded engine taxonomy.

The `guidance` mapping contains exactly:

| Key | Type | Required | Default |
|---|---|---:|---|
| `use_when` | non-empty sequence of unique non-empty Markdown strings | yes | none |
| `avoid_when` | non-empty sequence of unique non-empty Markdown strings | yes | none |
| `distinguish_from` | mapping from flavour name to non-empty Markdown string | no | `{}` |

Every `distinguish_from` key shall name another flavour declared by the same
schema. A flavour cannot distinguish itself. Guidance is descriptive authoring
policy: it helps humans and agents choose an item flavour, but it does not create
a traceability rule, validation rule, lifecycle transition, or implicit graph
edge. Projects define those independently through their schema declarations.

MID, flavour, source location, and document placement remain structural and
cannot be declared. The names `mid`, `flavour`, `id`, `title`, `body`,
`source_location`, and `mentions` are reserved and cannot be field or relation
names.

### Built-in declarations

The `id` mapping contains:

| Key | Type | Required | Default |
|---|---|---:|---|
| `required` | boolean | no | `false` |
| `pattern` | regex string | no | no pattern |

The `title` and `body` mappings each contain only optional boolean `required`,
defaulting to `false`. Empty strings do not satisfy a required `id`, `title`, or
body. An ID pattern is evaluated against the complete display ID.

### Field declarations

Each field key is a non-reserved `snake_name`. A field mapping contains only:

| Key | Type | Required | Default |
|---|---|---:|---|
| `type` | string | yes | none |
| `required` | boolean | no | `false` |
| `repeatable` | boolean | no | `false` |
| `values` | non-empty sequence of unique strings | only for `enum` | none |
| `pattern` | regex string | no; only for `string` | no pattern |

The closed type set is `string`, `integer`, `number`, `boolean`, and `enum`.
`values` is required for `enum` and forbidden for other types. `pattern` is
permitted only for `string`. v1 has no default, nullable, object, or nested type.

Metadata values are trimmed of surrounding ASCII space and tab for semantic
conversion while raw bytes remain in provenance:

- `string` is the trimmed UTF-8 text and may be empty only when not otherwise
  constrained;
- `enum` is exact case-sensitive equality with one declared value;
- `integer` matches `-?(?:0|[1-9][0-9]*)` and must fit signed 64-bit range;
- `number` uses the JSON number grammar, converts to a finite IEEE-754 binary64,
  and rejects overflow, `NaN`, and infinity;
- `boolean` is exactly lowercase `true` or `false`.

A non-repeatable field may occur zero or one time. A repeatable field may occur
zero or more times. `required: true` changes the minimum occurrence count to one.
Repeated semantic values retain source order and are not deduplicated.

:::

:::design m_01KY82WX4NJZSTBQNM2P2N06YM
:id: DES-SCHEMA-RELATION-DECLARATIONS
:title: Relation declaration and authoring namespace model
:status: accepted
:kind: data_model
:satisfies: REQ-SCHEMA-RELATIONS
:satisfies: REQ-SCHEMA-INVERSES

## Relation declarations

Each relation key is its canonical `snake_name`. A relation mapping contains
only:

| Key | Type | Required | Default |
|---|---|---:|---|
| `source` | endpoint mapping | yes | none |
| `target` | endpoint mapping | yes | none |
| `inverse` | `snake_name` string | no | none |
| `inverse_authoring` | boolean | no | `false` |
| `symmetric` | boolean | no | `false` |
| `same_flavour` | boolean | no | `false` |
| `self_reference` | boolean | no | `true` |
| `acyclic` | boolean | no | `false` |
| `cardinality` | cardinality mapping | no | unbounded |

The `source` endpoint contains a required non-empty `flavours` sequence and may
contain `derived`, a sequence from the v1 closed set `[source_span]`. The `target`
endpoint shall contain at least one of `flavours` or `external`; each present
sequence must be non-empty. Flavour names must resolve in the same schema.
External entries are schemes without `://`. Unknown endpoint keys are errors.

`inverse_authoring: true` requires `inverse`. A symmetric relation forbids
`inverse` and `inverse_authoring`, and its source and target flavour sets must be
identical; symmetric presentation uses the canonical name in both directions.
`same_flavour` requires internal source and target items to have equal flavours.
`acyclic` is allowed only when all targets are internal. A self-edge is rejected
when `self_reference` is false before cardinality or cycle evaluation.

The optional cardinality mapping contains `outgoing` and/or `incoming`. Each is
a mapping with optional non-negative integer `min` and `max`; `max` may instead
be the string `many`. Defaults are `min: 0` and `max: many`, and a numeric maximum
must be greater than or equal to the minimum. Outgoing counts canonical targets
per source item; incoming counts canonical sources per target item. Counts use
deduplicated normalized edges, not repeated authoring occurrences.

For each flavour, fields and authorable relations share one namespace. A
canonical relation name is authorable on every declared source flavour. An
inverse name is authorable on every declared target flavour only when
`inverse_authoring` is true. These names must not collide with built-ins, fields,
another canonical authoring name, or another enabled inverse on that flavour.

:::

:::design m_01KY82WX4PXWE6Y9EF4HJ5ADE0
:id: DES-SCHEMA-RULE-DECLARATIONS
:title: Validation rule declarations and compilation order
:status: accepted
:kind: data_model
:satisfies: REQ-SCHEMA-RULES

## Validation rule declarations

`rules` is ordered, and each rule name is unique. Every rule contains these
common keys:

| Key | Type | Required |
|---|---|---:|
| `name` | `snake_name` string | yes |
| `kind` | rule-kind string | yes |
| `severity` | `error`, `warning`, or `info` | yes |
| `applies_to` | mapping with non-empty `flavours` | yes |
| `when` | condition mapping | no |

Every `applies_to.flavours` entry must resolve. `when` contains exactly `field`
and `in`; `field` must exist on every applied flavour and `in` is a non-empty
sequence of distinct values. Each condition value must be valid for at least one
applied flavour. Values form a union when applied flavours use different enum
domains: a value absent from a particular flavour's enum can never match an item
of that flavour but does not invalidate the declaration. A rule is evaluated
only when the item's typed value equals an entry. Repeated condition fields are
invalid in v1 rather than using implicit any/all behaviour.

The v1 rule-kind set and additional keys are closed:

### `requires_relation`

Exactly one of `relation` or `relation_any_of` is required. `relation` is a
canonical relation name. `relation_any_of` is a non-empty unique sequence of
canonical names. `direction` is required and is `outgoing` or `incoming`. `min`
is a required non-negative integer; optional `max` is a non-negative integer or
`many` and cannot be below `min`. The rule counts the union of matching
deduplicated canonical edges.

### `requires_field`

Exactly one of `field` or `field_any_of` is required. `field` is one field name;
`field_any_of` is a non-empty unique sequence. Every named field must exist on
every applied flavour. `min` is a required non-negative integer and counts
present values across the selected fields. Optional `max` has the same form as
relation maximum. Structural built-ins are not fields for this rule.

### `orphan`

`relations` is a required non-empty unique sequence of canonical relation names.
An item fails when it participates in no selected canonical edge in either
direction. `when` may restrict the lifecycle values to which the check applies.
No `direction`, `min`, or `max` key is accepted.

Unknown rule kinds or keys are errors. Relation declarations enforce endpoint,
self-reference, cardinality, and acyclicity independently of project rules; a
project rule adds lifecycle- or flavour-specific policy and severity.

## Cross-declaration validation order

After structural decoding, Mara validates names and patterns, flavour and field
definitions, relation endpoint references and namespaces, then rule references.
It reports all independent schema defects. A schema with any error does not
produce a compiled semantic model, so content validation that depends on that
model is skipped rather than guessed.
:::
