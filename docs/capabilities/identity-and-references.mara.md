# Identity and references

Mara separates durable machine identity from mutable human handles and source
locations. Authors may use either form in references, but the normalized graph
always connects stable MIDs.

## Machine identity

:::req m_01KY7YA2FMFEGARGMEVHK9FSKC
:id: REQ-MID-STRUCTURAL
:title: Every item shall declare exactly one MID in its opening line
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-TRACEABILITY
:uses_term: TERM-MID

MID shall be mandatory structural syntax, not a metadata field. `:mid:` shall be
an unknown reserved metadata key, and an opening line with a missing, additional,
or malformed MID shall be invalid.
:::

:::req m_01KY7Y9R5TC2779JK3FE7VGVF8
:id: REQ-MID-FORMAT
:title: Mara shall support prefixed ULID machine identities
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: GOAL-AUDITABLE

v0.1 MID values shall contain the configured prefix followed by a canonical
uppercase 26-character ULID. The self-hosting schema uses the prefix `m_`.
:::

:::req m_01KY7Y9R5V3WZ75AKR4N52VDDH
:id: REQ-MID-GENERATE
:title: Mara shall generate collision-resistant MIDs
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-INITIALIZE-PROJECT

`mara mid` shall generate one valid MID using the project's configured prefix,
the current ULID timestamp component, and operating-system randomness. It shall
perform project discovery and load schema identity without loading content, and
print only the MID in human mode so scripts and authors can insert it directly.
Outside a valid Mara project it shall fail rather than assume an implicit prefix.
:::

:::req m_01KY7Y9R5WJP6Q1CYBMJ0HTWFM
:id: REQ-MID-UNIQUE
:title: Mara shall reject duplicate MIDs project-wide
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

No two authored items may share an MID. A duplicate-MID diagnostic shall report
the MID and every conflicting source location.
:::

:::req m_01KY7Y9R5XKVGT1H6KTBRHCEQ4
:id: REQ-MID-IMMUTABLE
:title: MID shall remain the durable identity across item changes
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

Changes to an item's flavour, display ID, title, fields, relations, body, or file
location shall not change its MID. v0.1 shall establish this source contract;
historical mutation detection may be added to revision-aware validation later.
:::

## Display IDs

:::req m_01KY7Y9R5Y5HSQ2AD40VDBVC3B
:id: REQ-DISPLAY-ID-SCHEMA
:title: Display-ID rules shall be flavour-configurable
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC
:uses_term: TERM-DISPLAY-ID

Display IDs shall be optional at the platform level. Each flavour shall declare
whether an ID is required and may define its exact case-sensitive pattern.
:::

:::req m_01KY7Y9R5Z1DRQX7T07Q6XQDVY
:id: REQ-DISPLAY-ID-UNIQUE
:title: Active display IDs shall be unique across the project
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-VALIDATE-PROJECT
:mitigates: RISK-DISPLAY-ID-COLLISION

Every present display ID shall be unique across all flavours. Duplicate IDs
shall be reported with every MID and source location because references to the
duplicate token are ambiguous.
:::

:::req m_01KY7Y9R60SJTXA94QJQGAQ4SK
:id: REQ-REFERENCE-RESOLUTION
:title: Mara shall resolve internal references deterministically
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

Mara shall first recognize a syntactically valid project MID and otherwise seek
an exact active display ID. Aliases, case folding, fuzzy matching, and heuristic
fallbacks shall not be supported. Unresolved and ambiguous references shall be
errors.
:::

## Mentions and typed relations

:::req m_01KY7Y9R610T51EM05G7GRVETW
:id: REQ-REFERENCE-BARE
:title: Bare wiki references shall create weak mentions
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

`[[REF]]` and `[[REF|label]]` shall resolve an internal or permitted external
target and create a weak mention. The optional label affects presentation but
not target resolution. For a bare external reference, the permitted scheme set
shall be the union of every external target scheme declared by any relation in
the active project schema. A scheme absent from that union shall be rejected.
:::

:::req m_01KY7Y9R62X0VVEHY4K7361CVA
:id: REQ-REFERENCE-TYPED
:title: Relation-qualified wiki references shall create typed edges
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: GOAL-TRACEABILITY

Inside an authored item, `[[relation:REF]]` and
`[[relation:REF|label]]` shall create the same semantic relation as a metadata
entry with that relation name. The qualifier shall resolve to a relation or
authorable inverse declared for the source flavour.
:::

:::req m_01KY7Y9R63GW2A6R7RBPQR363D
:id: REQ-RELATION-CANONICAL
:title: Mara shall normalize internal relations to canonical MID edges
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-TRACEABILITY

Every resolved authored internal relation shall be stored in the schema-declared
canonical direction with MID source and target endpoints, regardless of whether
the author used display IDs, MIDs, metadata, typed prose, or an inverse name. A
relation emitted by a permitted derived source scanner shall instead use its
project-relative source-span identity as the canonical source and the resolved
item MID as its target.
:::

:::req m_01KY7Y9R64EM55DZJGD7MRT8B9
:id: REQ-RELATION-BACKLINKS
:title: Mara shall derive inverses and backlinks
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

Mara shall derive inverse presentation and backlinks from canonical edges. It
shall never require authors to persist both directions or create a second
durable edge for an inverse.
:::

:::req m_01KY7Y9R65NAP4ZREZ8AJ7RXPM
:id: REQ-RELATION-DUPLICATES
:title: Mara shall deduplicate semantic edges while preserving provenance
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-AUDITABLE

Multiple source occurrences that normalize to the same edge shall produce one
semantic edge with all occurrence spans retained as provenance. An exact
duplicate metadata entry shall additionally produce a warning.
:::

## External objects

:::req m_01KY7Y9R66A4GCQPY08S8J95ZR
:id: REQ-EXTERNAL-URI
:title: Mara shall identify external objects with URI-shaped references
:status: approved
:level: system
:kind: interface
:priority: should
:derives_from: STORY-LINK-DELIVERY
:mitigates: RISK-REFERENCE-AMBIGUITY

External object identifiers shall use URI-shaped forms such as
`linear://MARA-123` and `github://owner/repository/issues/42`. The `://`
separator shall distinguish an external URI from a typed relation qualifier.
:::

:::req m_01KY7Y9R67GS4MEJN9HZ9CSFPQ
:id: REQ-EXTERNAL-CONSTRAINTS
:title: Schemas shall constrain external relation targets
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-LINK-DELIVERY

A typed relation may resolve to an external object only when that relation's
target declaration explicitly permits the object's URI scheme. Bare external
mentions may use the project-level union of schemes permitted by at least one
relation target. External objects shall be derived index nodes, not authored Mara
items.
:::

## Display-ID renaming contract

:::req m_01KY7Y9R685V918RPEFFRFTRY2
:id: REQ-DISPLAY-ID-RENAME
:title: Mara shall provide repository-wide display-ID renaming
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-RENAME-DISPLAY-ID

`mara id rename <old> <new>` shall validate the replacement against the target
flavour and project uniqueness rules, then rewrite every resolved internal
reference that uses the old display ID. MID references and unrelated matching
text shall remain unchanged.
:::

:::req m_01KY7Y9R69P4KETJ6N072VJEPB
:id: REQ-DISPLAY-ID-NO-ALIASES
:title: Mara shall not retain display-ID aliases
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

After a successful rename the previous display ID shall stop resolving. Mara
shall not provide display-ID aliases. Durable external references to Mara items
should therefore use MID because Mara cannot rewrite external systems or
historical revisions.
:::

:::req m_01KY7Y9R6A4H8XA75PBH8EBV6G
:id: REQ-REFERENCE-FAILURES
:title: Mara shall report reference failures at each occurrence
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

For each unresolved, ambiguous, disallowed, or source-incompatible reference,
Mara shall report its owning item or narrative span, source location, written
token, attempted relation when present, and failure reason.
:::

## Design, rationale, risks, and artifact

:::design m_01KY7Y9R6FZF1ZCN95BM2FJKPC
:id: DES-IDENTITY-INDEX
:title: Identity resolver and canonical edge index
:status: accepted
:kind: data_model
:satisfies: REQ-MID-STRUCTURAL
:satisfies: REQ-MID-FORMAT
:satisfies: REQ-MID-UNIQUE
:satisfies: REQ-MID-IMMUTABLE
:satisfies: REQ-DISPLAY-ID-SCHEMA
:satisfies: REQ-DISPLAY-ID-UNIQUE
:satisfies: REQ-REFERENCE-RESOLUTION
:satisfies: REQ-REFERENCE-BARE
:satisfies: REQ-REFERENCE-TYPED
:satisfies: REQ-RELATION-CANONICAL
:satisfies: REQ-RELATION-BACKLINKS
:satisfies: REQ-RELATION-DUPLICATES
:satisfies: REQ-EXTERNAL-URI
:satisfies: REQ-EXTERNAL-CONSTRAINTS
:satisfies: REQ-DISPLAY-ID-RENAME
:satisfies: REQ-DISPLAY-ID-NO-ALIASES
:satisfies: REQ-REFERENCE-FAILURES

The resolver shall build validated MID and display-ID tables before resolving
references. It shall emit canonical edges, weak mentions, external nodes,
derived inverses, and per-occurrence provenance as separate but connected
domain structures.
:::

:::decision m_01KY7Y9R6HB7ZE9DAV37SBWNXA
:id: ADR-0004
:title: Use prefixed ULIDs for machine identity
:status: accepted
:kind: architecture
:justifies: DES-IDENTITY-INDEX
:justifies: REQ-MID-FORMAT
:justifies: REQ-MID-GENERATE

ULIDs provide decentralized collision-resistant creation, compact URL-safe text,
and sortable creation-time prefixes without coupling identity to content,
location, flavour, or a central sequence allocator.
:::

:::decision m_01KY7Y9R6JXWHJ67F5YW081YNJ
:id: ADR-0005
:title: Rewrite display-ID references instead of preserving aliases
:status: accepted
:kind: process
:justifies: REQ-DISPLAY-ID-RENAME
:justifies: REQ-DISPLAY-ID-NO-ALIASES

Display IDs remain current human handles, while MID provides historical and
external durability. Repository-wide source-span-aware rewriting avoids an
ever-growing alias namespace and keeps current documents internally consistent.
:::

:::risk m_01KY7Y9R6K6XVH4JCY0G2B1T57
:id: RISK-DISPLAY-ID-COLLISION
:title: Parallel changes may introduce the same display ID
:status: open
:severity: medium
:likelihood: high
:affects: REQ-DISPLAY-ID-UNIQUE
:affects: REQ-DISPLAY-ID-RENAME

Independent branches may each introduce a valid item with the same human ID.
The merge retains distinct MIDs but creates an ambiguous human handle until the
collision is resolved.
:::

:::risk m_01KY7Y9R6M4T8S6HJVC45259Z0
:id: RISK-REFERENCE-AMBIGUITY
:title: Compact reference syntax may be parsed ambiguously
:status: open
:severity: medium
:likelihood: low
:affects: REQ-REFERENCE-TYPED
:affects: REQ-EXTERNAL-URI

A colon can qualify a typed relation while external identifiers also require a
scheme, so the same compact token prefix can otherwise admit two interpretations.
:::

:::artifact m_01KY7Y9R6NAJXWG2HVCHD4QYG9
:id: ART-MARA-INDEX
:title: Mara normalized JSON index
:status: proposed
:kind: index
:uri: .mara/index.json

The rebuildable JSON projection records identities, complete documents, items,
canonical edges, mentions, external nodes, and source provenance.
:::

## Planned verification

:::test m_01KY7Y9R6BCZ0P71W6PQ5WZMXK
:id: TEST-MID-IDENTITY
:title: MID generation and identity validation test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-MID-STRUCTURAL
:verifies: REQ-MID-FORMAT
:verifies: REQ-MID-GENERATE
:verifies: REQ-MID-UNIQUE
:verifies: REQ-MID-IMMUTABLE

Fixtures and command tests shall cover missing, malformed, lowercase, duplicate,
and generated MIDs and shall confirm that non-identity item changes preserve the
same MID.
:::

:::test m_01KY7Y9R6CG5F2S9E9GM2NGEZQ
:id: TEST-DISPLAY-ID
:title: Display-ID validation and rename test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-DISPLAY-ID-SCHEMA
:verifies: REQ-DISPLAY-ID-UNIQUE
:verifies: REQ-DISPLAY-ID-RENAME
:verifies: REQ-DISPLAY-ID-NO-ALIASES

Fixtures shall cover optional and required IDs, flavour patterns, project-wide
collisions, successful current-source rewriting, untouched MID references,
unrelated text, and failure of the old ID after rename.
:::

:::test m_01KY7Y9R6D4GCM93AK64927SGF
:id: TEST-REFERENCE-RESOLUTION
:title: Internal reference and relation normalization test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-REFERENCE-RESOLUTION
:verifies: REQ-REFERENCE-BARE
:verifies: REQ-REFERENCE-TYPED
:verifies: REQ-RELATION-CANONICAL
:verifies: REQ-RELATION-BACKLINKS
:verifies: REQ-RELATION-DUPLICATES
:verifies: REQ-REFERENCE-FAILURES

Fixtures shall combine MID and display-ID targets, labels, metadata and inline
relations, inverse authoring, duplicate occurrences, broken targets, ambiguous
IDs, invalid source flavours, and exact occurrence provenance.

The ambiguous-reference oracle shall emit `reference.ambiguous`, preserve the
ambiguous occurrence as its primary span, preserve the exact authored target
token in `details.reference`, list every candidate MID in ascending UTF-8 byte
order in `details.candidate_mids`, and list the corresponding item-header spans
in `related` in the same order.
:::

:::test m_01KY7Y9R6EKT6JMX09K40WG7AF
:id: TEST-EXTERNAL-REFERENCES
:title: External URI reference test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-EXTERNAL-URI
:verifies: REQ-EXTERNAL-CONSTRAINTS

Fixtures shall derive the project-level bare-mention scheme allowlist as the
union of all relation target schemes and cover bare and typed Linear and GitHub
URIs, a scheme permitted only on another relation, a scheme absent from the
union, relations that forbid external targets, and the distinction between URI
schemes and relation qualifiers without performing network access.
:::
