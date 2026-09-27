# Document structure and direct navigation

Items, narrative, sections and documents share one disposable source-backed
graph. These contracts own structure, references, node handles and direct
navigation. Search selection, ranking and complete node reads follow the
[retrieval contracts](retrieval.mara.md).

:::mara requirement REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
:mid: 01M232S32V718GRMEHSBPY46CQ
:title: Explore direct connections from items and narrative Markdown blocks
:status: accepted
:kind: functional
:derives_from: SCN-READ-DOCUMENT-CONTEXT

From an item, section or Markdown block returned by search, an actor can inspect
direct outgoing and incoming connections through CLI `related` or MCP `related`.
Return connection kind, direction, neighbour and source evidence. Preserve
parallel mention occurrences and distinguish mentions, authored schema relations
and structural containment. Narrative acquires neither a flavour nor neighbouring
items' metadata or relations.

Expose direct parent/child membership so actors can select sibling context through
successive calls. Each call is bounded with explicit continuation; it must not
expand another hop, assemble a path, or claim a complete trace. There is no `hops`
parameter. A returned reference can be passed to `get` or another `related` call.

Use `schema:` and `builtin:` to disambiguate relation names. Reject ambiguous
unqualified filters based on the schema vocabulary, even when the selected node
has no conflicting edges. Namespace qualification does not create a new edge.
:::

:::mara design DES-DOCUMENT-STRUCTURE
:mid: 01M234WMS5522HC5HDV42NG886
:title: Retain Markdown item containers and navigable section structure
:status: accepted
:kind: structure
:satisfies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

## Markdown projection

The parser adapter retains Mara items as containers with ordinary Markdown body
children under [[DES-DOCUMENT-FORMAT]]. Delimiter and mention recognition uses
whole-document Markdown context before container parsing; narrative and valid
item bodies share reference definitions, regardless of definition placement.
Heading scopes remain local to their item or Markdown container.

`Document::blocks()` and `Item::body_blocks()` expose Mara-owned blocks, including
GFM tables. Preserve nested children, heading levels, original source and UTF-8
byte spans. Table rows cover authored lines; cell spans exclude surrounding
separators/whitespace; padded cells have empty spans at the row content end. The
table owns its separator row. Bound containers at siblings and scope ends,
including the final line without a newline. Omit escaping children rather than
assigning neighbouring bytes or truncating an escaping table to fit. Invalid
metadata or incomplete item structure exposes no body blocks during recovery.

## Sections and graph

A heading opens a section ending before the next heading of the same or higher
importance, or at its scope's end. Lower headings open subsections; skipped
levels create no invented parents. Item headings cannot close outer sections.
Content before a heading belongs directly to its enclosing document or block.
Section extent includes its content; the original heading retains its own span.
Heading text decodes escapes and named/numeric entities once in ordinary text;
inline code remains literal. Retain decoded-text offsets into original Markdown.

`Corpus::discovery()` builds a disposable petgraph graph in document-path and
structural source order. Borrowed nodes expose kind, source, parent, children and
directional connections; graph indexes never escape. `contains` joins direct
parent to child; `contained_by` is its reverse view, with the same provenance.
Structure is distinct from schema relations, needs no authored identity and
implies no semantic dependency. Sibling navigation requires a parent call then a
children call. No recursive or automatic sibling edges are added.

Link sources belong to their owning item, otherwise their outermost ordinary
Markdown container. Destinations remain precise sections or blocks even inside
items. `Document::references()` retains parsed mentions, link destinations and
anchor declarations with exact locations, including unresolved references.

## Links and anchors

Bare item mentions resolve exact human IDs or MIDs. Markdown links without a
fragment resolve to documents; fragments resolve to sections or explicit anchors.
Resolve relative paths from the linking document, including `.` and `..`; a
leading slash selects a project-relative path. Decode path and fragment URL
escapes once. URI schemes, network-path URLs and links to non-Mara assets remain
source content without graph destinations or network reads.

Generated heading anchors lowercase decoded text, replace spaces with hyphens,
remove punctuation except hyphen/underscore, and retain letters, numbers and
combining marks. Allocate duplicate suffixes `-1`, `-2`, etc. across the whole
document, including item headings; already allocated names remain reserved.
Explicit anchors accept `<a name="value"></a>` with either quote style. Inline
anchors target the containing discovery block. A standalone anchor immediately
before a heading targets its section; before another block it targets that block.
Do not attach across item/container boundaries or a section end. In a shared
HTML block, placement is assessed for each declaration, not the whole block.

Resolved references produce `mentions` and derived backlinks carrying the same
source span. Ambiguous anchors and broken internal destinations produce
`reference_unresolved` errors and no resolved edge; source remains unchanged.
Mentions take precedence over Markdown reference definitions. Code, raw contexts
and escaped reference openings remain inert under [[DES-DOCUMENT-FORMAT]].

## Reusable references and summaries

An item reference is its MID; exact human IDs also resolve. Recovery items lacking
a MID remain addressable by ID while validation reports the defect. Other nodes
use opaque versioned handles derived from project-relative document path, current
source hash, kind and byte span. They survive process restarts and unrelated
document edits; any edit or move of their containing document invalidates them.
Stale or malformed handles fail with a rediscovery instruction. Git commits and
process-local graph indexes do not participate. Source-identical structural nodes
such as padded empty cells may share a handle.

The shared summary contains `reference`, `kind`, exact `source`,
`title_truncated`, and `context` references to the direct parent and nearest
enclosing section when present. Item summaries add ID/MID/flavour and title;
sections add title/heading level; blocks add block kind. Omit inapplicable fields.
Only titles truncate, at 256 Unicode scalars. Do not truncate identities, paths,
locations or context references. Pagination must fail rather than silently omit
a node whose mandatory summary cannot fit the response budget.

Item mutations must preserve the identity target of untouched surviving links.
Rename rewrites supported item-ID mentions in narrative; incoming references
block unsafe deletion. Candidate-graph preflight and Markdown-link retargeting follow
[[DES-ITEM-CREATION]], [[DES-ITEM-UPDATE]], [[DES-ITEM-DELETION]],
[[DES-ITEM-MOVEMENT]] and [[DES-ITEM-RENAME]].
:::

:::mara decision ADR-PETGRAPH-DISCOVERY
:mid: 01M2335G69NFYNP2J5BBBEGFYY
:title: Use petgraph for the private discovery graph
:status: accepted
:justifies: DES-DOCUMENT-STRUCTURE

Use petgraph's directed node/edge storage and directional iteration for shared
adjacency across items, narrative, sections and documents. Parallel connection
kinds and backlinks are immediate needs; a custom adjacency implementation would
duplicate them. Mara owns identity, reference resolution, connection meaning,
source provenance, deterministic ordering and pagination. Keep graph indexes
private and adapt results to Mara-owned values; persist no graph store.
This choice claims neither a measured speedup nor improved text-search relevance.
:::

:::mara decision ADR-MARKDOWN-STRUCTURAL-DISCOVERY
:mid: 01M234WMSE4STR6JPWZYHGQC1R
:title: Build discovery on Markdown containers and visible structural context
:status: accepted
:justifies: DES-DOCUMENT-STRUCTURE

Treat Mara as a Markdown extension with real item containers and derived section
hierarchy. Interleaved prose and items need visible parentage so actors can select
surrounding context without inventing authored section IDs or CRUD operations.
Keep search matches inside an item owned by that item while retaining precise
source locations and destinations within it. The `:::` grammar is Mara-owned,
not a CommonMark standard; this decision preserves the established item syntax.
:::

:::mara verification VER-DOCUMENT-NAVIGATION
:mid: 01M3FWZWMH99GSRHKJ08RF1SZQ
:title: Check document structure, references and direct navigation
:status: accepted
:method: test
:level: system
:verifies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS
:verifies: DES-DOCUMENT-STRUCTURE
:validates: SCN-READ-DOCUMENT-CONTEXT
:verifies: REQ-ITEM-RELATED
:verifies: REQ-RELATED-PAGINATION
:verifies: DES-DIRECT-NAVIGATION
:verifies: DES-CANONICAL-TRACE-RELATIONS

Run `cargo test --locked --test discovery --test discovery_handles --test references --test navigation` against the candidate. Structure/reference/handle checks preserve local scopes, Unicode/CRLF/EOF spans, exact link evidence, inert contexts, deterministic handles and unchanged source. Real CLI/stdin MCP searches narrative, pages its mentions, follows an item relation, reads the destination and navigates parent/children. Library projections alone do not establish this workflow.

Navigation checks consume complete count/byte-limited pages, preserve compact Unicode summaries, reject stale/invalid requests, namespace ambiguity, unsupported operation names and options and oversized mandatory entries, and retain source bytes. Verify alias/inline deduplication, canonical symmetric and self-edge identity before paging, exact terminal external addresses, shared code-marker/item-inverse edges, binary endpoints, file-only invalidation and selected code-marker/target failures. Use isolated fixture-owned identities and adapter packs. [[VER-RELATION-INSPECTION]], [[VER-CORPUS-CONFORMANCE]] and [[VER-POLICY-VALIDATION]] own occurrence, semantic and policy checks; mutation methods own source edits. Run the full regression suite, formatting and Clippy. Record candidate results from named read-only repository traversals at the tested revision.
:::

:::mara design DES-DIRECT-NAVIGATION
:mid: 01M3H1P241E4MZYGH75AA6JWGE
:title: Navigate bounded direct document and semantic connections
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-RELATED
:satisfies: REQ-RELATED-PAGINATION
:satisfies: REQ-DIRECT-KNOWLEDGE-NEIGHBOURS

CLI `related <reference>` and MCP `related {reference}` accept exact item IDs/MIDs, structural handles and code references. Optional direction, relations, flavours, limit and cursor share CLI/MCP semantics. Return discovery format 2 with node, connections, has_more and next_cursor. Built-in connections retain relation/direction/neighbour/source; schema connections carry canonical relation, label, direction, neighbour, canonical edge and occurrence_count. Internal neighbours reuse [[DES-DOCUMENT-STRUCTURE]] summaries; external neighbours have only kind external and exact address. They cannot be get/related roots.

Resolve canonical names and inverse aliases in the schema namespace; built-ins are contains and mentions. Accept schema:/builtin: qualification; reject ambiguous short names based on vocabulary even without matching edges. Alias selection never reverses direction. Relation choices are ORed; neighbour flavours are ORed and intersect other filters, selecting item neighbours only. Missing or ambiguous selected authored item/code targets and invalid selected code-source markers fail rather than silently completing the traversal.

Order outgoing, incoming, then symmetric connections. Retain deterministic neighbour path/source order and built-in occurrence order; external outgoing targets sort by address/relation. Code incoming connections merge into source order; code outgoing targets sort by item source. Deduplicate schema/self edges under [[REQ-RELATED-PAGINATION]] before bounds. Incoming schema labels use the inverse alias when available; human output omits the incoming prefix then, otherwise displays it. Human built-in incoming containment displays contained_by; JSON keeps contains with incoming direction.

Default limit 20, accepted range 1–100, counts connections rather than neighbours. Keep each JSON domain result within 65,536 UTF-8 bytes including cursor/escaping, excluding transport wrappers. Reject an oversized mandatory root/next connection without skipping it. Continuation binds reference, direction, original filter values, limit and shared source/schema/code invalidation under [[DES-ITEM-LIST]]. Reject changed, malformed, initial or out-of-range positions with a restart instruction. No hops, recursive expansion, item related or item_related. All navigation is read-only.
:::

:::mara requirement REQ-ITEM-RELATED
:mid: 01M1PXP2KG97XHSEEB4KCZVTAP
:title: Retrieve compact directly related items
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

CLI/MCP `related` returns a selected item's direct incoming, outgoing and symmetric semantic connections, with canonical relation, displayed label, neighbour and occurrence count. Include supported code backlinks and terminal external targets. Exact relation and neighbour-flavour filters combine with direction without expanding another hop. Schema and built-in connections stay distinct under [[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]]. Follow [[DES-DIRECT-NAVIGATION]] for bounded transport and [[DES-CANONICAL-TRACE-RELATIONS]] for identity.
:::

:::mara requirement REQ-RELATED-PAGINATION
:mid: 01M1RY3MCMCZK45K50SS5TRYBX
:title: Continue bounded direct-neighbour results
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Return bounded direct-connection pages with explicit continuation. Filter and deduplicate semantic schema edges before bounds; distinct relation kinds remain separate entries and parallel built-in mentions retain their occurrences. Consume every page without omissions or duplicates. A directed self-edge appears once outgoing by default or once in the explicitly requested orientation; symmetric self-edges appear once only for omitted/symmetric direction. Preserve stable ordering and reject stale or invalid requests. Neighbour bodies require get. Bounds and cursor semantics follow [[DES-DIRECT-NAVIGATION]].
:::

:::mara design DES-CANONICAL-TRACE-RELATIONS
:mid: 01M2FX575WG9SP8EZJSTEE7VG9
:title: Normalize authoring forms while retaining their occurrences
:status: accepted
:kind: structure
:satisfies: REQ-ITEM-RELATED
:satisfies: REQ-CODE-TRACEABILITY

Build a disposable semantic relation graph alongside [[DES-DOCUMENT-STRUCTURE]]. Endpoints are items identified by MID, code by exact project-relative code reference, and external targets by exact authored address. Canonical name and directed endpoints identify one edge; symmetric item endpoints sort by MID solely for stable identity. Different relation kinds stay distinct. Inverse aliases exchange authored endpoints before validating declared source/target/same-flavour constraints. Metadata, typed inline and code-marker equivalents contribute occurrence counts to one edge. Invalid assertions create no graph edge; validation remains a separate capability.

Internal edge endpoints require MIDs. A code source uses a canonical relation permitting code and the target flavour; item-to-code assertions require its inverse alias and [[DES-CODE-READ]] resolution. External targets require a permitted canonical external relation and literal `external:` plus absolute HTTP(S) with a host, without credentials, whitespace/control characters, raw brackets/angle brackets or backslashes. Preserve exact address spelling; no fetching, URL normalization, synthetic MID/flavour or outgoing external graph.

Authored occurrences retain their parsed document/code source spans. [[DES-RELATION-INTERFACES]] exposes their locations and snapshot-bound selectors; [[DES-RELATION-MUTATION]] defines source edits. No reverse assertion is written merely to provide incoming navigation.
:::

:::mara design DES-RELATION-INTERFACES
:mid: 01M2GC4PXK0MMANAW6AKGXAW7S
:title: Expose canonical relationships and bounded occurrence inspection
:status: accepted
:kind: interface
:satisfies: REQ-RELATION-INSPECTION

CLI `relation get SOURCE RELATION TARGET [--limit N] [--cursor TOKEN]` and MCP `relation_get {source,relation,target,limit?,cursor?}` resolve canonical edge identity under [[DES-CANONICAL-TRACE-RELATIONS]]. Accept internal IDs/MIDs, canonical code sources or item-side inverse code targets, and permitted external targets. No source mutation occurs. Unknown parameters are rejected.

Return relationship format 1 with `edge`, total `occurrence_count`, `occurrences`, `has_more` and nullable `next_cursor`. Each occurrence contains opaque `reference`, `kind` (metadata, inline or code_comment), exact source path/byte/line location, authored endpoint, original relation spelling and original target scalar. Preserve aliases and ID/MID/external spelling. Sort all occurrences by source path then start byte before pagination, including code comments among document assertions. Derived reverse navigation does not create an occurrence.

Default limit 20, accepted range 1–100. Domain JSON including escaping/cursor must fit 65,536 UTF-8 bytes; transport wrappers are outside the budget. Never truncate fixed identity/location/spelling fields or silently skip an oversized occurrence; return a page_limit error if the next occurrence cannot fit. Missing edges return relation_not_found with the resolved edge and zero count. Other failures use the relationship error envelope `{format_version:1,error:{code,message},edge?,occurrence_count?}`; CLI exits unsuccessfully and MCP marks isError while retaining structured content.

Occurrence selectors bind project root and source/schema/code snapshot while preserving each authored occurrence's identity within it. Continuation additionally binds source/relation/target spelling and limit; alias-equivalent requests must restart rather than exchange cursors. Reject stale, malformed, initial and out-of-range continuation with a restart instruction. Reinspection after source/schema changes produces new selectors. Selectors are not persisted identities. Their use for removal and the add/remove interfaces follow [[DES-RELATION-MUTATION]]; direct navigation follows [[DES-DIRECT-NAVIGATION]].
:::

:::mara requirement REQ-RELATION-INSPECTION
:mid: 01M3H27927R2KSCQ087RK317K1
:title: Inspect every authored occurrence of a canonical relationship
:status: accepted
:kind: functional
:derives_from: SCN-READ-DOCUMENT-CONTEXT

An actor can inspect an existing canonical relationship through CLI and MCP and recover every authored occurrence without changing source. Equivalent canonical/inverse, ID/MID and symmetric requests identify the same edge. Preserve metadata, typed-inline and code-comment source locations, author identity, original relation spelling and target scalar. Return total count and bounded consecutive pages in source path/byte order; unchanged traversal has no omissions or duplicates. Missing edges and invalid requests are explicit structured errors. Format, bounds and snapshot-bound selectors follow [[DES-RELATION-INTERFACES]].
:::

:::mara verification VER-RELATION-INSPECTION
:mid: 01M3H27DSGRH032NZFPCBKDERM
:title: Verify relation inspection through CLI and MCP
:status: accepted
:level: system
:method: test
:verifies: REQ-RELATION-INSPECTION
:verifies: DES-RELATION-INTERFACES

Run `cargo test --locked --test relation_inspection` against real CLI and stdio MCP with isolated, fixture-owned source, identities and adapters. Verify canonical/alias/ID/MID and symmetric equivalence, exact metadata/inline/code spans and spelling, external addresses, complete count/byte-limited pages, total counts, snapshot-bound selectors and unchanged source bytes. Reproduce a code path sorting before its item document; require global path/byte order before pagination. Check changed request/source/schema cursors, malformed positions, oversized occurrences, missing edges, invalid endpoints/limits and unknown parameters with equivalent structured errors. [[VER-RELATION-MUTATION]] owns source-write checks; [[VER-CORPUS-CONFORMANCE]] owns semantic validation. Run the full regression suite, formatting, Clippy, canonical validation and selected traceability. Inspect actual repository edge occurrences without modifying source.
:::
