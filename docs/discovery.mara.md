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
| Filters | Project-relative path filters apply to all result kinds. Item ID, flavour, custom-field, and schema-relation filters select items only. Narrative never inherits item metadata. Omit node-kind filters (`--kind` or an MCP equivalent) in 0.2; keep node kinds in result output. |
| Connections | Explicit resolved references create `mentions` edges, with incoming backlinks. Expose derived structural membership and direct parent/child connections under [[DES-DOCUMENT-STRUCTURE]]. Preserve source locations and distinguish structural connections, mentions, and schema-defined typed relations. |
| Reading | Replace `mara item get` with `mara get <reference>` for items, sections, Markdown blocks, and documents, with equivalent MCP retrieval. Accept item IDs/MIDs and discovery handles. Return node kind, source location, structural context, bounded consecutive content, and item metadata when applicable. Provide continuation for complete reads; enumerate neighbours through `related`. File tools remain optional. |
| Navigation | Replace CLI `mara item related` with `mara related <reference>` for items and structural discovery nodes, with equivalent unified MCP navigation. Accept item IDs/MIDs and returned discovery handles. Follow [[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]] from every result kind. Excerpts support selection; they are not a claim to include all connected context. |

Keep `search`, `get`, and `related` at the CLI top level. Group them as
"Discovery and reading" in help and documentation without adding a command
namespace. Item creation, update, rename, move, deletion, list, and validation
remain under `item` and accept items only. `relation add/remove` continue to
author schema-defined relations between items; structural connections and
mentions are derived from document structure and links.

Markdown blocks can participate in the discovery graph without becoming
schema-defined items. Graph membership does not infer semantic obligations or
implementation claims from prose. Raw URLs and code links remain source content
until their target-resolution contract exists. Code-symbol extraction and
richer traceability are later work, not prerequisites for 0.2.

Unified search matches heading text as section results, ordinary content
outside items as Markdown block results, and content within items as owning
item results, including their nested headings. A section hit identifies the
whole section for reading and navigation; it does not match merely because a
child block contains the query. This keeps heading discovery available without
requiring actors to choose Markdown node types before searching.

Discovery relation names distinguish `schema:` from `builtin:`. Resolve an
unqualified name when it exists in exactly one namespace: `satisfies` resolves
to `schema:satisfies`, and `contains` resolves to `builtin:contains` unless
the schema also declares it. If both declare a name, reject the unqualified
form and identify the fully qualified alternatives. Explicit names select their
namespace. Resolve against the available vocabulary, not the connections
present in a result, so shorthand meaning is stable across queries.

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

## Ranking and response bounds

Retain normalization and word-level typo tolerance under
[[REQ-FUZZY-ITEM-SEARCH]]. Every distinct query term must match the result's own
searchable content. Results matching every term exactly precede any result
requiring typo tolerance. Within each group, sum each term's highest matching
field weight: item ID, title, or heading = 3; body and other metadata = 1.
Headings inside an item contribute to that item's score. Parent section titles
provide context without making their child blocks match. Repeated occurrences
add no weight. Break ties by document path and source order; give no score
bonus for node kind, connection count, or document length.

Carry forward [[REQ-RETRIEVAL-BOUNDS]] for 0.2 with these changes and extensions:

| Area | 0.2 contract |
|---|---|
| Search and related pages | Default 20 entries; `limit` accepts 1 through 100. Related counts connections, including distinct relations to the same neighbour. |
| Response bytes | At most 65,536 UTF-8 bytes per serialized JSON domain result, including escaping and continuation metadata; transport wrappers remain outside the budget. |
| Search excerpts | Include one source excerpt of at most 240 Unicode scalar values per hit by default. Mark omitted content; use `get` for complete reading. This replaces 0.1's opt-in excerpts and maximum of three. |
| Summary titles/headings | At most 256 Unicode scalar values; mark truncation. Complete text remains retrievable through `get`. |
| Node reading | Return consecutive content and applicable metadata up to the response budget, then explicit continuation. Preserve complete handles and source locations. |

Apply filters and ordering before pagination; the byte budget may shorten a
page below its entry limit. Preserve the existing no-silent-skip rule when
mandatory fields cannot fit. Large blocks remain single nodes: paginate their
content, not their identity. Content and metadata fragments must reconstruct
complete values without gaps or duplication, respecting Unicode boundaries.

Remove the relation-count `--limit` option and equivalent MCP parameter from
`get`, which no longer enumerates neighbours. Retain its continuation cursor;
`search` and `related` retain both limit and cursor.

Verify mixed-result ranking, exact-before-fuzzy order, equal title/heading
weights including headings inside items, absence of ancestor-title inheritance
and repetition bonuses, stable ties, default excerpts, byte-limited pages, and
complete consecutive reads of oversized nodes.

## Discovery response format

CLI JSON and MCP use the same domain result with `format_version: 1`.
Version this discovery response contract independently from schema and
application versions. MCP tools are `search`, `get`, and `related`, matching
the top-level CLI commands. MCP `get` and `related` accept `reference`
instead of `id`; project selection remains unchanged.

Use one node summary across operations:

| Fields | Meaning |
|---|---|
| `reference`, `kind` | Reference accepted by get/related; kind is item, section, block, or document. |
| `source` | Project-relative path, start/end byte offsets, and start/end lines for the complete node. |
| `title`, `title_truncated` | Item title or section heading where applicable, with explicit truncation. |
| `context` | Direct parent and nearest section references when present, without recursive expansion. |
| `id`, `mid`, `flavour` | Items only. |
| `block_kind` | Blocks only. |
| `heading_level` | Sections only. |

All responses include `has_more` and `next_cursor`, null when no content
remains. Their operation-specific fields are:

| Operation | Fields |
|---|---|
| search | `results: [{node, excerpt}]` |
| get | `node`, `content`, `content_range`, `metadata`, `metadata_range` |
| related | `node`, `connections: [{relation, direction, neighbour, source}]` |

For items, content is the parsed body; other nodes return their original
Markdown span, including contained source for sections and documents.
`content_range` uses the existing body-relative text-range shape, and metadata
keeps the ordered fragment/range semantics of [[DES-RETRIEVAL-CONTINUATION]].
Non-items have empty metadata. Excerpts retain source text, locations, and
partial markers under the limits above.

A connection's `relation` is one string using the same ambiguity rule as
input: emit the short name if unique in the available vocabulary, otherwise
`builtin:name` or `schema:name`. Direction is relative to the requested
node; neighbour uses the shared summary, and source locates the connection's
evidence. JSON uses canonical `contains` with direction; human output renders
its incoming view as `contained_by`.

## Migration from 0.1

0.2 removes the old discovery names without aliases. Other item, relation,
schema, and project command names remain unchanged.

| Old usage | 0.2 replacement |
|---|---|
| CLI `item search/get/related`; MCP `item_search/item_get/item_related` | Top-level CLI and MCP `search/get/related` |
| MCP get/related `id` argument | `reference` |
| Search `--excerpts` / MCP `excerpts` | Remove; one excerpt is automatic |
| Get `--limit` / MCP `limit` | Remove; neighbour limits belong to related |
| Search `items`; related `items` | `results`; `connections` using shared node summaries |
| Get `summary`, `body`, `body_range` | `node`, `content`, `content_range`; source is in node |
| Get incoming/outgoing neighbour collections | Read through related |

For example, CLI `mara item get REQ-RETRY` becomes
`mara get REQ-RETRY`. The equivalent MCP invocation changes from
`item_get({"id":"REQ-RETRY"})` to `get({"reference":"REQ-RETRY"})`.

Discard old cursors on upgrade and update response parsers for mixed node
kinds and the discovery format above. Keep the migration guide to this mapping,
the CLI/MCP example, and the cursor note; link to the response contract instead
of duplicating field definitions.

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

Use top-level `search`, `get`, and `related` for discovery and reading.
Bounded node retrieval lets actors search, read, inspect direct connections,
and read a selected neighbour through Mara regardless of node kind. Requiring
a switch to filesystem tools for Markdown content would interrupt this same
workflow, especially for MCP-only actors. File tools remain an optional route.

This replaces separate document/passage search and the earlier 0.2 decision
requiring file tools for narrative reads. Lack of durable item identity does
not exclude a Markdown block from the graph or bounded retrieval. Keep item
authoring commands under `item` so that "item" consistently means an authored
Mara item; another namespace for the primary read workflow adds no useful
distinction.

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

The adapter first recognizes delimiters and mentions in full-document Markdown
context so multiline code spans and raw blocks retain the existing syntax
boundaries. It then parses each recognized item as a Rushdown container owning
its identity, ordered metadata, body, and closing delimiter. Narrative and valid
item bodies share one document-wide Markdown reference-definition context,
including definitions before or after an item; heading scopes remain local.
Ordinary body blocks, including GFM tables, are projected into Mara-owned `MarkdownBlock`
values with block kind, source location, and nested block children, available
through `Item::body_blocks()`. Headings retain their level. Table row spans
cover their authored lines; cell spans cover their Markdown content without
surrounding separators or whitespace. Padded cells in short rows have empty
spans at the row's content end. The table owns its separator row; header and
cell spans do not include it. Rushdown types stay private; reads and edits use
the original source rather than rendered AST text. Bound container spans at
following siblings; preserve the final line at EOF even without a trailing
newline. Omit parser children positioned outside their enclosing
source range rather than assigning neighbouring bytes to them. Apply these
bounds to complete table spans too; do not truncate an escaping table into a
valid-looking child. Use UTF-8 byte boundaries for every exposed source span.
Validation recovery retains partial item data but exposes no body blocks for
items with invalid metadata or incomplete structure.

## Derived sections

Headings remain Markdown nodes. Derive sections from heading levels within their
containing scope: a section ends before the next heading of the same or higher
importance, or at the containing scope's end. Lower-importance headings open
subsections. Accept skipped levels without inventing missing headings; H1 after
H3 ends the H3 section rather than nesting inside it. An item's body has its own
heading scope, so headings inside it cannot close outer document sections.
Other Markdown container boundaries must likewise preserve their own children.
Content before a heading belongs directly to its containing document or block.

A section carries its heading text, level, and source location. Heading text
decodes Markdown backslash escapes and named/numeric character references once
in ordinary text nodes; code spans retain their literal content. Sections contain
ordinary Markdown blocks, items, and subsections in source order. Prose before
and after an item can belong to the same section. Add no abstract passage
container around those blocks or special passage node for a heading.

The Rust projection retains narrative blocks through `Document::blocks()` and
heading text through `MarkdownBlock::heading_text()`. `Corpus::discovery()` builds
a disposable petgraph graph from that loaded snapshot. Borrowed `DiscoveryNode`
values expose node kind, source, parent, children, and directional connections;
sections retain their original heading block and a source span covering their
full extent. Graph indexes stay private. Schema relations, resolved item and narrative
references, and containment share this graph. `Document::references()` retains
item mentions, Markdown link destinations, and explicit anchor declarations
with precise source spans, including unresolved links. Link sources use their
owning item or outermost ordinary Markdown block; link destinations retain
sections and blocks inside items. `DiscoveryGraph::diagnostics()` reports broken
internal references and ambiguous anchors; project validation includes these
schema-independent diagnostics. Discovery handles, search selection, and CLI/MCP
discovery remain separate work.

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
does not introduce synthetic passage nodes. Ranking and response bounds follow
[[DES-UNIFIED-KNOWLEDGE-DISCOVERY]], which also defines response and continuation fields.

## Item mutation and link safety

In 0.2, preflight all item mutations against the candidate source structure.
Reject operations that would break or silently retarget an untouched surviving
link, and report affected source locations before writing any files. Explicitly
edited links may change destination subject to validation; references removed
with deleted content do not count as surviving links. Rename retains automatic
rewriting of supported item-ID references while preserving their identity
targets, including references in ordinary Markdown.

Movement preserves ID/MID references because the item's identity is unchanged.
Check incoming links to sections or blocks inside the moved item, relative or
same-document links carried with it, and generated heading anchors affected in
either document. Same-document moves can also change numbered heading anchors.
Creation, update, and deletion must likewise preserve destinations of untouched
surviving links when heading anchors shift.

Do not automatically repair Markdown links as a side effect of these operations
in 0.2. Authors must resolve reported link impacts before retrying. This extends
the current [item editing](editing.mara.md) contracts for the new discovery
model; it does not change the 0.1 implementation.

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
