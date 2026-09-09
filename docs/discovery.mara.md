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
or relations. Document/section containment is structural context, not another
authored semantic relation.

Each call returns direct neighbours only, with bounded results and explicit
continuation. There is no `hops` parameter, recursive expansion, automatic path
assembly, or claim that returned neighbours form a complete trace. Actors may
request another node's direct neighbours themselves. Richer graph analysis and
traceability remain future 0.3/0.4 scope.

Verify a narrative search hit leading through a mention to an item and through
that item's typed relation to another item, as successive calls. Verify the
corresponding incoming connections and continuation without silently expanding
an additional hop.
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
| Passage extraction | Use ordinary Markdown structure from Rushdown through the Mara-owned adapter. Headings and item boundaries inform passage grouping; retain heading context and intact Markdown constructs. Item bodies are searched as items, not duplicated as narrative candidates. No new authoring markers or flavour are required. |
| Result kinds | Distinguish items from passages explicitly. Both are searchable, addressable discovery nodes with excerpts and source locations. Items retain their IDs/MIDs; a passage handle locates source in a particular revision and is not a permanent item identity. |
| Filters | Project-relative path filters apply to both kinds. Item ID, flavour, custom-field, and schema-relation filters select items only. Narrative never inherits item metadata. |
| Connections | Explicit resolved references create `mentions` edges, with incoming backlinks. Preserve the written link's source location and distinguish mentions from schema-defined typed relations. Document paths and heading hierarchy provide structural context. |
| Source reading | Return locations sufficient for the actor's existing file-reading tools. This workflow assumes access to the same project sources. Do not add generic document `list`/`get` operations or recreate plain file reading. Existing structured item operations remain separately useful. |
| Navigation | Follow [[REQ-DIRECT-KNOWLEDGE-NEIGHBOURS]] from either result kind. Excerpts support selection; they are not a claim to include all connected context. |

Passages can participate in the discovery graph without becoming schema-defined
items. Graph membership does not infer semantic obligations or implementation
claims from prose. Raw URLs and code links remain source content until their
target-resolution contract exists. Code-symbol extraction and richer
traceability are later work, not prerequisites for 0.2.

Implementation details still to settle: exact Markdown passage grouping and
oversized-block treatment; passage handles and stale-location rejection;
Markdown destination/anchor resolution (including passage targets and broken or
ambiguous destinations); mixed-result ranking and wire fields; continuation
limits; neighbour operation naming; and migration/alias policy for the existing
CLI/MCP search names. Supporting both `[[ID]]` mentions and resolvable Markdown
links is intended; exact resolution rules must be specified before
implementation.

This design specifies a discovery graph, not a graph-engine dependency or
persisted graph store. The existing disposable-projection boundary remains:
source documents own meaning, and parser/library node indexes must not become
public identities.
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
derived-mention concepts are useful precedent, but its petgraph choice, bounded
multi-hop traversal, and broader traceability contracts are not adopted by this
decision. Richer graph analysis and code traceability remain provisional
0.3/0.4 work.
:::
