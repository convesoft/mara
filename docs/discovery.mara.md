# Unified knowledge discovery

Accepted 0.2 direction: search canonical documentation once, then inspect
direct connections from items, sections, and ordinary Markdown blocks. These contracts
extend the [guided-authoring scope](guided-authoring.mara.md); they are not
implemented by the [0.1 retrieval contract](retrieval.mara.md).

:::mara requirement REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
:mid: 01M232S32V718GRMEHSBPY46CQ
:title: Explore direct connections from items and narrative Markdown blocks
:derives_from: SCN-READ-DOCUMENT-CONTEXT

In 0.2, an actor can start from an item, section, or Markdown block returned by
search and inspect its direct outgoing and incoming connections through CLI or
MCP. Return the connection kind, direction, neighbouring node, and source
location so the actor can choose the next step and read the evidence.

Resolved explicit references from narrative produce `mentions` edges and
derived incoming backlinks. Schema-defined typed relations remain authored on
items; narrative does not acquire a flavour or inherit adjacent items' metadata
or relations.

Expose structural membership for item and Markdown block results. Actors can
navigate direct parent/child connections through derived sections and documents
to select possible sibling context. These built-in connections are distinct
from authored semantic relations. Structure and item ownership follow
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
successive parent/child navigation to a sibling narrative Markdown block.
:::

:::mara design DES-UNIFIED-KNOWLEDGE-DISCOVERY
:mid: 01M232SX5VJZ65J1ZGGKZ556S9
:title: Search items and Markdown blocks through one discovery surface
:satisfies: REQ-DOCUMENT-CONTEXT-DISCOVERY
:satisfies: REQ-DOCUMENT-CONTEXT-READ
:satisfies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Accepted direction for 0.2; not implemented by 0.1.

| Concern | Contract |
|---|---|
| Entry point | Move CLI search to `mara search`, with equivalent unified MCP discovery. Search covers items, sections, and ordinary Markdown blocks in the selected project's canonical documents, including documents without items. Do not introduce a second document-search operation. |
| Document structure | Follow [[DES-DOCUMENT-STRUCTURE]] for Rushdown item containers, derived sections, Markdown block selection, and owning-item search results. |
| Result kinds | Return item, section, or Markdown block results, exposing the block's specific kind. All are addressable discovery nodes with excerpts and source locations. Items retain their IDs/MIDs; a Markdown block handle locates source in a particular revision and is not a permanent item identity. |
| Filters | Project-relative path filters apply to all result kinds. Item ID, flavour, custom-field, and schema-relation filters select items only. Narrative never inherits item metadata. |
| Connections | Explicit resolved references create `mentions` edges, with incoming backlinks. Expose derived structural membership and direct parent/child connections under [[DES-DOCUMENT-STRUCTURE]]. Preserve source locations and distinguish structural connections, mentions, and schema-defined typed relations. |
| Source reading | Return locations sufficient for the actor's existing file-reading tools. This workflow assumes access to the same project sources. Do not add generic document `list`/`get` operations or recreate plain file reading. Existing structured item operations remain separately useful. |
| Navigation | Replace CLI `mara item related` with `mara related <reference>` for items and structural discovery nodes, with equivalent unified MCP navigation. Accept item IDs/MIDs and returned discovery handles. Follow [[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]] from every result kind. Excerpts support selection; they are not a claim to include all connected context. |

Markdown blocks can participate in the discovery graph without becoming
schema-defined items. Graph membership does not infer semantic obligations or
implementation claims from prose. Raw URLs and code links remain source content
until their target-resolution contract exists. Code-symbol extraction and
richer traceability are later work, not prerequisites for 0.2.

Link and anchor resolution follows [[DES-DOCUMENT-STRUCTURE]].

Generate versioned, opaque discovery handles deterministically from the
project-relative document path, a hash of its current source contents, node
kind, and start/end byte offsets. Apply this to blocks, sections, and documents;
items retain durable MIDs. Handles require neither authored IDs nor persistent
storage. Identical inputs produce identical handles across commands and
restarts. Editing or moving the containing document invalidates its old
handles; reject them and instruct the actor to search again. Use working-file
contents, including uncommitted edits, rather than a Git commit.

Changes to another document leave a node handle valid; its connections reflect
the current corpus. Pagination cursors retain broader source/schema and request
invalidation because other documents can change result membership and ordering.
The token encoding and hash algorithm are implementation details; do not expose
private graph indexes as handles.

Implementation details still to settle: representation of built-in connection
kinds versus schema relations;
mixed-result ranking, wire fields, excerpt and continuation limits; MCP operation naming;
and migration/alias policy for the existing CLI/MCP search
names.

The private in-memory graph backend follows [[ADR-PETGRAPH-DISCOVERY]]. The
existing disposable-projection boundary remains: source documents own meaning,
and parser/library node indexes must not become public identities.
:::

:::mara decision ADR-UNIFIED-KNOWLEDGE-DISCOVERY
:mid: 01M232SX66VXH8FG5JW68G6R3S
:title: Make narrative part of unified discovery and direct navigation
:justifies: DES-UNIFIED-KNOWLEDGE-DISCOVERY
:justifies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Use one search surface for structured items and ordinary narrative. Markdown
blocks are first-class discovery nodes whose explicit references connect them
to items and provide backlinks. Require no authored flavour or MID for
narrative. Actors choose each next direct neighbour; 0.2 has no hop-count
parameter or automatic traversal.

An actor may find an explanation first, follow its mention to a requirement,
then inspect a connected design or decision. Discovery must work from that
entry point without requiring the actor to know which content kind contains the
answer. Keep connection kinds and source locations so the actor can understand
why each neighbour matters.

This replaces the earlier preference for separate document/passage search and
the proposal to implement generic document reads in Mara. Existing
source-reading tools provide the content once Mara locates it. Lack of durable
item identity does not exclude a Markdown block from the graph.

The 0.1 item-only contract remains unchanged. The POC's narrative-span and
derived-mention concepts are useful precedent. The private backend is decided
separately in [[ADR-PETGRAPH-DISCOVERY]]; the POC's multi-hop traversal and
broader traceability contracts are not adopted here. Richer graph analysis and
code traceability remain provisional 0.3/0.4 work.
:::

:::mara decision ADR-PETGRAPH-DISCOVERY
:mid: 01M2335G69NFYNP2J5BBBEGFYY
:title: Use petgraph for the private 0.2 discovery graph
:justifies: DES-UNIFIED-KNOWLEDGE-DISCOVERY
:justifies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

Adopt petgraph when implementing the 0.2 discovery graph. Use a private
directed representation for items, Markdown blocks, derived sections, and
documents, with typed relations, resolved mentions, and structural connections
under [[DES-DOCUMENT-STRUCTURE]]. Enumerate incoming and outgoing edge
references for direct-neighbour queries; derive backlinks from those edges
rather than authoring inverse links. Support distinct relation kinds between
the same endpoints.

The immediate need is shared adjacency storage and direct navigation across
knowledge and structural nodes. Reuse the library's node/edge storage and
directional iteration instead of maintaining an equivalent custom graph. Future
graph algorithms reinforce this choice but are not the sole justification. The
[petgraph Graph
API](https://docs.rs/petgraph/0.8.3/petgraph/graph/struct.Graph.html) supports
associated node/edge data, parallel edges, and directional edge iteration;
dependency version selection belongs to implementation.

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

A section carries its heading text, level, and source location. Sections contain
ordinary Markdown blocks, items, and subsections in source order. Prose before
and after an item can belong to the same section. Add no abstract passage
container around those blocks or special passage node for a heading.

## Discovery units

Return one of three result categories: item, section, or Markdown block. Expose
the specific Markdown block kind. Retaining an AST child does not require
returning that child as an independent search hit.

| Match location | Result unit |
|---|---|
| Anywhere inside a Mara item, including its nested headings and sections | Owning item, with the actual match location. |
| Section heading outside an item | Section. |
| Standalone paragraph outside an item | Paragraph. |
| List content, including nested lists | Outermost containing list. |
| Table cell content | Whole table. |
| Code-block content | Whole code block. |
| Blockquote content | Whole blockquote. |

Item ownership takes precedence. Otherwise retain the outermost ordinary
Markdown block container: for example, a list within a blockquote returns the
blockquote. Do not duplicate the same match as both its child and enclosing
result unit. Large blocks remain single nodes with bounded excerpts; size alone
does not introduce synthetic passage nodes. Excerpt size and continuation wire
fields remain to be specified.

## Connections and containment

### Links and anchors

Resolve item references to items, a Markdown link without a fragment to its
document, and a heading fragment to its section. Section destinations cover
their full structural extent without automatically retrieving children.
Same-document fragments and cross-document links use the same anchor rules.
Resolve relative paths from the linking document, including `./` and `../`:
`[retry policy](./architecture.mara.md#retry-policy)` targets a section in
another document in the project.

Use [GitHub-compatible heading anchors](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#section-links):
lowercase heading text, replace spaces with hyphens, remove punctuation and
formatting, and number duplicate generated anchors with `-1`, `-2`, and so on.
Allocate heading anchors across the whole document, including headings inside
items; heading nesting remains scoped as specified above. Renaming or
reordering headings can change these generated destinations.

Support explicit anchors written as `<a name="retry-policy"></a>`.
An anchor inside ordinary content targets its containing discovery block and
retains its precise source location. A standalone anchor immediately before a
heading targets that section; before another block it targets that block.
Do not attach anchors across an item or section boundary. Explicit anchors
provide a stable alternative to generated heading names. Search grouping and
item ownership must not erase a link's precise destination.

Resolved links produce `mentions` connections and derived incoming backlinks.
Broken internal destinations and ambiguous anchors are validation errors;
preserve the written link but create no resolved connection. External URLs
remain source links without network validation. Code-link and other unresolved
target categories retain the scope boundary in [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].

Verify same-document and relative cross-document section links, whole-document
links, duplicate heading names including headings inside items, explicit anchor
placement, and broken or ambiguous internal destinations.

Documents and sections are addressable structural nodes. Results expose their
structural parent and section context when present. Actors can inspect direct
parent/child connections and select sibling context. Derive containment and its
reverse view from source structure. Keep these built-in connections distinct
from mentions and schema-authored typed relations. Require no authored IDs,
flavours, metadata, or dedicated section/document authoring operations.

Use `contains` from direct parent to child and `contained_by` for its reverse
view. These are two directions of one derived structural connection, describing
immediate containment rather than all descendants. The connection representation must distinguish
built-in structural kinds from schema-defined relation names.

Sibling discovery remains successive direct navigation: inspect the item's
parent, then that parent's children. A shared parent indicates possible context,
not a semantic dependency. Add no automatic sibling relation, recursive
expansion, or hops parameter. Raw Markdown inline nodes need not become discovery
graph nodes.

Verify interleaved items and prose, nested lists/quotes, tables, headings inside
items, skipped heading levels, heading-free content, direct containment in both
directions, heading-link targets, and item ownership of nested matches,
alongside existing format and editing contracts.
:::

:::mara decision ADR-MARKDOWN-STRUCTURAL-DISCOVERY
:mid: 01M234WMSE4STR6JPWZYHGQC1R
:title: Build discovery on Markdown containers and visible structural context
:justifies: DES-DOCUMENT-STRUCTURE

Treat Mara as a Markdown extension: model items as real container blocks and
retain the Markdown structure inside and around them. Derive section hierarchy
for context and navigation instead of treating each heading as a narrative
Markdown block or attaching it only to the next prose fragment.

Interleaved Markdown blocks and items share section context. An actor must be
able to see that parentage and navigate to possible sibling context from either
kind of search result. Sections are internal in the sense that they are derived
and need no authored identities or CRUD operations; they are visible to actors
through locations and structural connections.

The POC already specified Markdown item bodies and a complete hierarchy of
sections, narrative blocks, and item placements. This decision accepts that
structural direction for 0.2 without copying the POC's old item syntax or
broader traceability scope. The `:::` form follows a fenced-container extension
convention; it is not syntax standardized by CommonMark itself. Mara owns its
exact grammar and restrictions.

Keep search results focused on the owning item when content matches within its
body, while retaining the actual source location and structural context. The
0.1 implementation remains unchanged by this planning decision.
:::
