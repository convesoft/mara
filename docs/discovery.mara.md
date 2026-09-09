# Unified knowledge discovery

Accepted 0.2 direction: search canonical documentation once, then inspect
direct connections from either an item or a narrative passage. These contracts
extend the [guided-authoring scope](guided-authoring.mara.md); they are not
implemented by the [0.1 retrieval contract](retrieval.mara.md).

:::mara requirement REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
:mid: 01M232S32V718GRMEHSBPY46CQ
:title: Explore direct connections from items and narrative passages
:derives_from: SCN-READ-DOCUMENT-CONTEXT

In 0.2, an actor can start from either an item or a narrative passage returned
by search and inspect its direct outgoing and incoming connections through CLI
or MCP. Return the connection kind, direction, neighbouring node, and source
location so the actor can choose the next step and read the evidence.

Resolved explicit references from narrative produce `mentions` edges and
derived incoming backlinks. Schema-defined typed relations remain authored on
items; narrative does not acquire a flavour or inherit adjacent items' metadata
or relations.

Expose structural membership for item and passage results. Actors can navigate
direct parent/child connections through derived sections and documents to select
possible sibling context. These built-in connections are distinct from authored
semantic relations. Structure and item ownership follow
[[DES-DOCUMENT-STRUCTURE]].

Each call returns direct neighbours only, with bounded results and explicit
continuation. There is no `hops` parameter, recursive expansion, automatic path
assembly, or claim that returned neighbours form a complete trace. Actors may
request another node's direct neighbours themselves. Richer graph analysis and
traceability remain future 0.3/0.4 scope.

Verify a narrative search hit leading through a mention to an item and through
that item's typed relation to another item, as successive calls. Verify the
corresponding incoming connections and continuation without silently expanding
an additional hop. Also verify an item's visible section membership and
successive parent/child navigation to a sibling narrative passage.
:::

:::mara design DES-UNIFIED-KNOWLEDGE-DISCOVERY
:mid: 01M232SX5VJZ65J1ZGGKZ556S9
:title: Search items and passages through one discovery surface
:satisfies: REQ-DOCUMENT-CONTEXT-DISCOVERY
:satisfies: REQ-DOCUMENT-CONTEXT-READ
:satisfies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Accepted direction for 0.2; not implemented by 0.1.

| Concern | Contract |
|---|---|
| Entry point | Move CLI search to `mara search`, with equivalent unified MCP discovery. Search covers items and narrative passages in the selected project's canonical documents, including documents without items. Do not introduce a second document-search operation. |
| Document structure | Follow [[DES-DOCUMENT-STRUCTURE]] for Rushdown item containers, derived sections, passage boundaries, and owning-item search results. |
| Result kinds | Distinguish items from passages explicitly. Both are searchable, addressable discovery nodes with excerpts and source locations. Items retain their IDs/MIDs; a passage handle locates source in a particular revision and is not a permanent item identity. |
| Filters | Project-relative path filters apply to both kinds. Item ID, flavour, custom-field, and schema-relation filters select items only. Narrative never inherits item metadata. |
| Connections | Explicit resolved references create `mentions` edges, with incoming backlinks. Expose derived structural membership and direct parent/child connections under [[DES-DOCUMENT-STRUCTURE]]. Preserve source locations and distinguish structural connections, mentions, and schema-defined typed relations. |
| Source reading | Return locations sufficient for the actor's existing file-reading tools. This workflow assumes access to the same project sources. Do not add generic document `list`/`get` operations or recreate plain file reading. Existing structured item operations remain separately useful. |
| Navigation | Follow [[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]] from either result kind. Excerpts support selection; they are not a claim to include all connected context. |

Passages can participate in the discovery graph without becoming schema-defined
items. Graph membership does not infer semantic obligations or implementation
claims from prose. Raw URLs and code links remain source content until their
target-resolution contract exists. Code-symbol extraction and richer
traceability are later work, not prerequisites for 0.2.

Implementation details still to settle: exact Markdown passage grouping and
oversized-block treatment; passage handles and stale-location rejection;
Markdown destination/anchor resolution (including section targets and broken or
ambiguous destinations); heading-only search results; structural handle/edge
names; mixed-result ranking and wire fields; continuation
limits; neighbour operation naming; and migration/alias policy for the existing
CLI/MCP search names. Supporting both `[[ID]]` mentions and resolvable Markdown
links is intended; exact resolution rules must be specified before
implementation.

The private in-memory graph backend follows [[ADR-PETGRAPH-DISCOVERY]]. The
existing disposable-projection boundary remains: source documents own meaning,
and parser/library node indexes must not become public identities.
:::

:::mara decision ADR-UNIFIED-KNOWLEDGE-DISCOVERY
:mid: 01M232SX66VXH8FG5JW68G6R3S
:title: Make narrative part of unified discovery and direct navigation
:justifies: DES-UNIFIED-KNOWLEDGE-DISCOVERY
:justifies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Use one search surface for structured items and ordinary narrative. Passages
are first-class discovery nodes whose explicit references connect them to items
and provide backlinks. Require no authored flavour or MID for narrative. Actors
choose each next direct neighbour; 0.2 has no hop-count parameter or automatic
traversal.

An actor may find an explanation first, follow its mention to a requirement,
then inspect a connected design or decision. Discovery must work from that
entry point without requiring the actor to know which content kind contains the
answer. Keep connection kinds and source locations so the actor can understand
why each neighbour matters.

This replaces the earlier preference for separate document/passage search and
the proposal to implement generic document reads in Mara. Existing
source-reading tools provide the content once Mara locates it. Lack of durable
item identity does not exclude a passage from the graph.

The 0.1 item-only contract remains unchanged. The POC's narrative-span and
derived-mention concepts are useful precedent. The private backend is decided
separately in [[ADR-PETGRAPH-DISCOVERY]]; the POC's multi-hop traversal and broader
traceability contracts are not adopted here. Richer graph analysis and code
traceability remain provisional 0.3/0.4 work.
:::

:::mara decision ADR-PETGRAPH-DISCOVERY
:mid: 01M2335G69NFYNP2J5BBBEGFYY
:title: Use petgraph for the private 0.2 discovery graph
:justifies: DES-UNIFIED-KNOWLEDGE-DISCOVERY
:justifies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Adopt petgraph when implementing the 0.2 discovery graph. Use a private directed
representation for items, passages, derived sections, and documents, with typed
relations, resolved mentions, and structural connections under
[[DES-DOCUMENT-STRUCTURE]]. Enumerate incoming and outgoing edge references for
direct-neighbour queries; derive backlinks from those edges rather than
authoring inverse links.
Support distinct relation kinds between the same endpoints.

The immediate need is shared adjacency storage and direct navigation across
knowledge and structural nodes. Reuse the library's node/edge storage and
directional iteration instead of maintaining an equivalent custom graph. Future graph
algorithms reinforce this choice but are not the sole justification. The
[petgraph Graph API](https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.Graph.html)
supports associated node/edge data, parallel edges, and directional edge
iteration; dependency version selection belongs to implementation.

Mara owns identity and reference resolution, connection meaning, source
provenance, schema validation, deterministic result ordering, and pagination.
Library node/edge indexes remain private and process-local. Adapt petgraph
results to Mara-owned types at the boundary. Preserve the disposable projection
of canonical sources; this decision introduces no persisted graph store.

This dependency is justified by implementation reuse, not a measured speedup.
Keyword matching, typo tolerance, and text ranking remain separate. A graph
backend does not provide a full-text index or automatically improve relevance.
0.2 remains direct-neighbour only, without a hops parameter; richer traversal,
traceability, and code-symbol extraction retain their later scope. Add no
runtime dependency or feature implementation to the 0.1 release preparation.
:::

:::mara design DES-DOCUMENT-STRUCTURE
:mid: 01M234WMS5522HC5HDV42NG886
:title: Retain Markdown item containers and navigable section structure
:satisfies: REQ-DOCUMENT-CONTEXT-DISCOVERY
:satisfies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Accepted for 0.2. Keep Markdown syntax, derived document structure, and search
result selection distinct.

## Item containers

Represent an entire Mara item as a custom Rushdown container block whose body
contains ordinary Markdown AST children. Mara parses its opening identity,
metadata, and closing delimiter; Rushdown parses the Markdown body. Preserve the
current authoring syntax, exact source spans, code/raw-context handling, and
rejection of nested items and malformed delimiters. Do not broaden allowed item
placement as a side effect of changing the parser representation.

The existing 0.1 adapter recognizes delimiter and mention extension nodes, then
pairs delimiters and extracts item ranges. 0.2 replaces this item projection
with the container model while keeping Rushdown APIs behind Mara-owned types
and preserving the original source for reads and edits.

## Derived sections

Headings remain Markdown nodes. Derive sections from heading levels within their
containing scope: a section ends before the next heading of the same or higher
importance, or at the containing scope's end. Lower-importance headings open
subsections. Accept skipped levels without inventing missing headings; H1 after
H3 ends the H3 section rather than nesting inside it. An item's body has its own
heading scope, so headings inside it cannot close outer document sections.
Other Markdown container boundaries must likewise preserve their own children.
Content before a heading belongs directly to its containing document or block.

A section carries its heading text, level, and source location. Its heading is
not an artificial narrative passage. Sections may contain narrative passages,
items, and subsections in source order; narrative before and after an item can
belong to the same section. Passage grouping operates on narrative Markdown
blocks within this structure and must not cross item or section boundaries.

## Discovery and containment

Documents and sections are addressable structural nodes in discovery navigation.
Search results expose an item's or passage's structural parent and section
context when present; actors can inspect direct parent/child connections and
select sibling context. Derive containment and its reverse view from document
structure. Keep these built-in structural connections distinct from mentions
and schema-authored typed relations; add no required authored IDs,
flavours, metadata, or dedicated section/document authoring operations.

A text match anywhere inside an item, including its nested sections, returns
the owning item with the precise matching source location, not a separate
passage result. Heading links identify their actual structural destination;
search result grouping must not redirect them to an arbitrary nearby passage.
A shared parent indicates possible context, not a semantic dependency.

Sibling discovery is successive direct navigation: inspect the item's parent,
then that parent's children. It does not require an automatic sibling relation,
recursive expansion, or a hops parameter. Raw Markdown inline nodes need not
become discovery graph nodes.

Before implementation, specify structural handle and edge wire names,
heading-only search results, anchor resolution, and passage grouping/size rules.
Verify interleaved items and prose, headings inside items, skipped heading
levels, heading-free content, direct containment in both directions, and item
ownership of nested matches, alongside existing format and editing contracts.
:::

:::mara decision ADR-MARKDOWN-STRUCTURAL-DISCOVERY
:mid: 01M234WMSE4STR6JPWZYHGQC1R
:title: Build discovery on Markdown containers and visible structural context
:justifies: DES-DOCUMENT-STRUCTURE

Treat Mara as a Markdown extension: model items as real container blocks and
retain the Markdown structure inside and around them. Derive section hierarchy
for context and navigation instead of treating each heading as a narrative
passage or attaching it only to the next prose fragment.

Interleaved passages and items share section context. An actor must be able to
see that parentage and navigate to possible sibling context from either kind
of search result. Sections are internal in the sense that they are derived and
need no authored identities or CRUD operations; they are visible to actors
through locations and structural connections.

The POC already specified Markdown item bodies and a complete hierarchy of
sections, narrative blocks, and item placements. This decision accepts that
structural direction for 0.2 without copying the POC's old item syntax or broader
traceability scope. The `:::` form follows a fenced-container extension
convention; it is not syntax standardized by CommonMark itself. Mara owns its
exact grammar and restrictions.

Keep search results focused on the owning item when content matches within its
body, while retaining the actual source location and structural context. The
0.1 implementation remains unchanged by this planning decision.
:::
