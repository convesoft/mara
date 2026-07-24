# Schema system

The schema is the project's semantic contract. It defines what authored item
flavours mean and which graph structures are valid, while the engine remains
independent of any requirements methodology or business taxonomy.

## Schema identity and strict loading

:::req m_01KY7Y9R4HK6P43G9JJJWEG5Z7
:id: REQ-SCHEMA-VERSION
:title: Mara shall version every schema format
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

Every schema shall declare a supported integer format version and a human schema
name and semantic version. Mara shall reject unsupported format versions rather
than guessing their meaning.
:::

:::req m_01KY7Y9R4JJZGS12GJHXWSEDDF
:id: REQ-SCHEMA-STRICT
:title: Mara shall validate schema documents strictly
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

Mara shall reject unknown keys, duplicate YAML mapping keys, invalid value
types, invalid names, conflicting field and relation keys, and internally
inconsistent declarations. Every diagnostic shall identify the schema source
location.
:::

:::req m_01KY7Y9R4KQ04EBD5EFF8R46T9
:id: REQ-SCHEMA-IDENTITY
:title: Mara shall configure MID representation at schema scope
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC
:uses_term: TERM-MID

The schema shall contain one project-wide MID representation with a format and
prefix. v0.1 shall accept only `format: ulid`, while retaining the format field
as an explicit compatibility boundary.
:::

## Flavours and fields

:::req m_01KY7Y9R4MWTKBXTEXAY1E3SGJ
:id: REQ-SCHEMA-FLAVOURS
:title: Mara shall define item flavours and their authoring guidance in the schema
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE
:uses_term: TERM-FLAVOUR

The schema shall declare each permitted lowercase ASCII snake-case flavour and
shall provide its human-facing label, semantic purpose, creation criteria,
exclusions, and distinctions from confusable flavours. The compiled schema shall
retain this structured guidance for human reference views, web interfaces, and
agent context assembly. Adding, removing, or documenting a project flavour shall
require no parser or engine code change.
:::

:::req m_01KY7Y9R4NDN5CCMK2YWC17QT5
:id: REQ-SCHEMA-BUILTINS
:title: Mara shall configure standard item built-ins per flavour
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Each flavour shall configure display-ID requiredness and pattern, title
requiredness, and body requiredness. MID, flavour, source location, and document
placement remain structural platform data and shall not be redefined as fields.
:::

:::req m_01KY7Y9R4PZ7AS8CJDET584RJX
:id: REQ-SCHEMA-FIELDS
:title: Mara shall support explicit scalar field declarations
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC

A flavour may declare local fields of type string, integer, number, boolean, or
enum. A field declaration may set requiredness, repetition, enum values, and a
string pattern. v0.1 shall not support nested field values or flavour
inheritance.
:::

:::req m_01KY7Y9R4QGH2ZTCZAB2MW5N5W
:id: REQ-SCHEMA-REPEATED-FIELDS
:title: Mara shall model repeated field values explicitly
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-AUTHOR-KNOWLEDGE

Only fields declared repeatable may occur more than once. Mara shall preserve
the source order of repeated values and shall reject duplicate occurrences of a
non-repeatable field.
:::

## Relations and graph constraints

:::req m_01KY7Y9R4RY717BFQ4HSB9QD1W
:id: REQ-SCHEMA-RELATIONS
:title: Mara shall define typed relations in the schema
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-TRACEABILITY
:uses_term: TERM-RELATION

A relation declaration shall define its canonical name, permitted source and
target flavours, permitted derived source kinds, permitted external target
schemes, cardinality, self-reference policy, symmetry, and optional acyclicity.
Field names and authorable relation names shall share one namespace within a
flavour.
:::

:::req m_01KY7Y9R4S0KQDX94BVRVJYKGM
:id: REQ-SCHEMA-INVERSES
:title: Mara shall declare inverse authoring names without duplicating edges
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-TRACEABILITY

A non-symmetric relation may declare one inverse name and whether that inverse
may be authored. Canonical and inverse occurrences shall normalize to one
canonical edge; backlinks shall be derived rather than persisted in source.
:::

:::req m_01KY7Y9R4T4TM7ET5FY28K88GH
:id: REQ-SCHEMA-RULES
:title: Mara shall support project-defined declarative validation rules
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC
:derives_from: STORY-VALIDATE-PROJECT

The schema shall configure field and relation requirements, conditional
requirements, cardinality, orphan detection, allowed graph endpoints,
self-reference, and acyclicity with project-selected severities. Mara shall not
hardcode a process or mandatory trace path.
:::

## Explicitness and composition boundary

:::req m_01KY7Y9R4VTRSG6KR144MG4N6X
:id: REQ-SCHEMA-NO-DEFAULTS
:title: Mara shall not synthesize missing field values
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-READABLE-SOURCE

v0.1 schemas shall not define implicit field defaults. A semantically relevant
authored value shall be visible in source, and an absent optional configuration
key shall remain absent in the source-preserving decoded representation. A
compiler may assign explicitly specified effective semantics to omission without
manufacturing an authored value or source span.
:::

:::req m_01KY7Y9R4WY8FBM94N4AHJ1EQJ
:id: REQ-SCHEMA-SINGLE-FILE
:title: Mara shall use one local schema file in v0.1
:status: approved
:level: system
:kind: constraint
:priority: should
:derives_from: GOAL-AUDITABLE

Each v0.1 project shall reference one Git-tracked local YAML schema. Schema
imports, remote packages, reusable mixins, and flavour inheritance are reserved
for a later version and shall not be accepted silently.
:::

## Design, rationale, risk, and artifact

:::design m_01KY7Y9R50YD4A79ZZ7H4B9C33
:id: DES-SCHEMA-META-MODEL
:title: Compiled strict schema meta-model
:status: accepted
:kind: data_model
:satisfies: REQ-SCHEMA-VERSION
:satisfies: REQ-SCHEMA-STRICT
:satisfies: REQ-SCHEMA-IDENTITY
:satisfies: REQ-SCHEMA-FLAVOURS
:satisfies: REQ-SCHEMA-BUILTINS
:satisfies: REQ-SCHEMA-FIELDS
:satisfies: REQ-SCHEMA-REPEATED-FIELDS
:satisfies: REQ-SCHEMA-RELATIONS
:satisfies: REQ-SCHEMA-INVERSES
:satisfies: REQ-SCHEMA-RULES
:satisfies: REQ-SCHEMA-NO-DEFAULTS
:mitigates: RISK-SCHEMA-COMPLEXITY
:satisfies: REQ-SCHEMA-SINGLE-FILE

The engine shall deserialize YAML into a version-specific compiled meta-model,
validate schema-internal references and namespaces, and expose immutable flavour,
field, relation, and rule definitions to later pipeline stages.
:::

:::decision m_01KY7Y9R514ZK53091RJ39ZM1P
:id: ADR-0002
:title: Keep process semantics in project schemas
:status: accepted
:kind: schema
:justifies: DES-SCHEMA-META-MODEL
:justifies: REQ-SCHEMA-RULES

Mara's core recognizes generic items and graph constraints only. Requirements,
stories, tests, V-model traces, Unified Process concepts, and all other business
objects belong to project schemas or future reusable schema packages.
:::

:::risk m_01KY7Y9R52JD2S9SCTPW5E340Z
:id: RISK-SCHEMA-COMPLEXITY
:title: Schema flexibility may create an unmaintainable language
:status: open
:severity: high
:likelihood: medium
:affects: DES-SCHEMA-META-MODEL
:affects: REQ-SCHEMA-RULES

Unbounded expressions, inheritance, defaults, and executable extensions could
make schemas difficult to review and produce non-deterministic behaviour across
implementations.
:::

:::artifact m_01KY7Y9R535A28YJAWCEH0Y51R
:id: ART-MARA-SCHEMA
:title: Mara self-hosting schema
:status: proposed
:kind: schema
:uri: .mara/schema.yaml

This YAML file defines the twelve Mara dogfooding flavours, their fields, the
project relation vocabulary, and progressive validation rules.
:::

## Planned verification

:::test m_01KY7Y9R4X92DY4XH356PG1JDK
:id: TEST-SCHEMA-STRICT
:title: Strict schema loading test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-SCHEMA-VERSION
:verifies: REQ-SCHEMA-STRICT
:verifies: REQ-SCHEMA-IDENTITY

Fixtures shall cover supported and unsupported versions, the allowed YAML
profile, an empty bootstrap flavour map, duplicate YAML keys, every unknown-key
position, invalid names and regexes, invalid identity settings, and
source-located diagnostics.
:::

:::test m_01KY7Y9R4YKEJKXCPZK3BFMYEZ
:id: TEST-SCHEMA-FLAVOURS-FIELDS
:title: Flavour and field meta-model test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-SCHEMA-FLAVOURS
:verifies: REQ-SCHEMA-BUILTINS
:verifies: REQ-SCHEMA-FIELDS
:verifies: REQ-SCHEMA-REPEATED-FIELDS
:verifies: REQ-SCHEMA-NO-DEFAULTS
:verifies: REQ-SCHEMA-SINGLE-FILE

Fixtures shall validate every scalar type, pattern, enum, required field,
repeatable field, duplicate non-repeatable field, flavour label, description,
creation criterion, exclusion, cross-flavour distinction, forbidden default,
and unsupported composition construct. They shall reject missing or empty
guidance, unknown distinction targets, and self-distinction.
:::

:::test m_01KY7Y9R4ZP3ZS5WPB2C9J2ZW4
:id: TEST-SCHEMA-RELATIONS-RULES
:title: Relation and rule meta-model test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-SCHEMA-RELATIONS
:verifies: REQ-SCHEMA-INVERSES
:verifies: REQ-SCHEMA-RULES

Fixtures shall cover endpoint constraints, external schemes, inverse namespace
collisions, symmetric relations, cardinality, self-reference, acyclicity,
conditional requirements across shared and different enum domains, every closed
rule shape, and orphan-rule declarations. Cardinality fixtures shall combine item,
derived source-span, and external `NodeRef` endpoints and verify counts per
materialized node identity, including authored items with zero edges.
:::
