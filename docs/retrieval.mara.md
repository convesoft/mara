# Enhanced deterministic retrieval

This document records the accepted scope for `0.1.0-alpha.3`, not implemented
behaviour. The retrieval contracts in [First alpha](alpha.mara.md) describe the
current baseline. The open choices below must be settled in the affected
contracts before implementation; this scope does not select a search library.

## Scope

Five implementation work areas, each including its contract updates, CLI/MCP
parity, and focused verification:

| Work area | Outcome |
|---|---|
| Search/list | Bounded summaries and pagination; optional search excerpts and selected-item filtering |
| Related items | Bounded direct-neighbour results with continuation |
| Item retrieval | Bounded consecutive body portions and relation lists with continuation |
| Fuzzy search | Typo-tolerant word matching |
| Relevance ranking | Stronger matches first with deterministic ordering |

Release preparation follows end-to-end verification of search, optional excerpt
inspection, item reading, and caller-selected relation traversal. Large titles,
large relation sets, Unicode text, and a body consisting of one enormous line
must exercise the response bounds. Real retrieval examples must demonstrate
typo recovery and useful ranking, including preservation of exact matches.

Traversal remains caller-controlled: inspect one item and its direct relations,
select relevant neighbours, and retrieve them as often as needed. No automatic
multi-hop expansion is added.

Semantic/hybrid search, embeddings or model execution, synonym dictionaries,
stemming, substring/prefix modes, a query language, persisted indexes or graph
stores, and context profiles are outside this milestone. A fuzzy match may
incidentally cover a word-form variation; it does not promise morphological or
substring matching. Existing project, schema, and authored document formats
remain the source of truth.

## Open implementation contracts

- **Bounds:** default and maximum counts and text sizes; size units; handling of
  oversized metadata, paths, and other fields; human/JSON/MCP truncation markers.
  A result-count limit alone cannot bound a response, and line limits alone
  cannot bound a one-line body.
- **Continuation:** request/result shapes and position representation; resuming
  body and relation portions independently; behaviour when source content or
  ranked results change between calls. Unchanged input must be retrievable
  without omissions or duplicate portions.
- **Search policy:** whether fuzzy matching and relevance ordering are defaults
  or explicitly selected; compatibility with current exact matching and corpus
  ordering; typo thresholds by term length; scoring and field weights.
- **Excerpts:** fragment selection and count, source-position format, behaviour
  when terms match different fields or passages, and selected-ID filter
  cardinality and invalid-handle behaviour. Reuse search matching semantics;
  do not introduce a separate interpretation of fuzzy matches for previews.

## Planned requirements

:::mara requirement REQ-RETRIEVAL-BOUNDS
:mid: 01M1RY3MB07B3QJM0603QHMHBM
:title: Bound retrieval responses explicitly
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Search/list summaries, related-item summaries, and item retrieval must have
finite response bounds. Cap displayed summary titles and indicate truncation;
preserve complete item handles for subsequent retrieval. Full titles remain
retrievable through item get. Bounds must handle Unicode and oversized single
lines without presenting omitted content as complete. The bound values and
oversized-field policy remain open above.
:::

:::mara requirement REQ-SEARCH-PAGINATION
:mid: 01M1RY3MBT9BFSXDPSS7D8ZKAV
:title: Continue bounded search and list results
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item search and item list must return bounded pages and explicitly indicate
whether more results are available. Callers can retrieve subsequent pages.
Apply filters and the selected ordering before pagination. For unchanged input,
following continuation retrieves the complete result set without omissions or
duplicates. Default results remain compact item summaries without body text.
:::

:::mara requirement REQ-RELATED-PAGINATION
:mid: 01M1RY3MCMCZK45K50SS5TRYBX
:title: Continue bounded direct-neighbour results
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item related must return bounded pages of direct incoming and outgoing
neighbours, retaining relation names, directions, and existing filters. Make
remaining results explicit and retrievable through continuation. Preserve
outgoing-before-incoming and corpus/authored relation ordering. Neighbour
bodies require explicit retrieval by the caller.
:::

:::mara requirement REQ-PARTIAL-ITEM-READ
:mid: 01M1RY3MDC8V9YE7744Z4CRY0Y
:title: Read large items in consecutive portions
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item get must return the complete body when it fits the response budget.
Otherwise return an explicitly partial, consecutive body portion with a way to
retrieve the remainder. For unchanged content, continuation reconstructs the
entire body without gaps or duplication, including a body with no line breaks.
Bound the incoming and outgoing relation lists returned by get as well and
make omitted entries retrievable. A partial read must not silently jump to
query matches or discard intervening text.
:::

:::mara requirement REQ-FUZZY-ITEM-SEARCH
:mid: 01M1RY3ME6RSKJ8DBXYN7VYTBX
:title: Recover word matches containing small spelling errors
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item search must support typo-tolerant word matching while retaining existing
scope filters and requiring every distinct query term to match. Use
conservative matching for short terms. Item-handle lookup and filter values
remain exact. This is word-level spelling tolerance, not file-picker-style
subsequence matching. Activation defaults and edit thresholds remain open.
:::

:::mara requirement REQ-SEARCH-RELEVANCE
:mid: 01M1RY3MEY6HVP2Z8AFVE7YAH8
:title: Rank search results reproducibly by relevance
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item search must support relevance ordering that favours exact over approximate
matches and accounts for matches in IDs and titles. Define scoring and field
weights before implementation. For unchanged input and options, ordering must
be reproducible with stable tie-breaking. Rank before limiting or paginating;
item list remains in corpus order. Whether relevance ordering is the search
default remains open.
:::

:::mara requirement REQ-SEARCH-EXCERPTS
:mid: 01M1RY3MFSN4GYSXVJJEMMTQD1
:title: Inspect bounded matching passages on request
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item search must return bounded excerpts only when explicitly requested.
Callers can restrict search to selected item handles to inspect their matches,
or request excerpts during the initial corpus search. Return source passages
and positions, not generated summaries. Mark excerpts as incomplete views;
they may skip intervening content and do not replace a complete item read.
Without the excerpt option, return the existing compact summary shape, subject
to the planned bounds and pagination metadata.
:::

:::mara design DES-SEARCH-EXCERPT-OPTIONS
:mid: 01M1RY4EGEC55K394TEZY5RYTR
:title: Expose excerpts through existing search operations
:satisfies: REQ-SEARCH-EXCERPTS

Extend `item search` and MCP `item_search` with equivalent options for excerpts
and exact selected-item filtering; do not add a separate preview operation.
The planned CLI spelling is `--excerpts` and `--id <id-or-mid>`. MCP field names,
selected-item multiplicity, and detailed result schemas remain to be specified.

The same query and matching rules apply with or without excerpts. A caller may
search the corpus for compact summaries, repeat the query restricted to a
selected item with excerpts, then use `item get` for a complete or consecutive
partial read. Initial searches may request excerpts directly to avoid an extra
call. Excerpt positions identify where to inspect the source; they are not a
promise that the excerpt contains the item's complete meaning.

Search still accepts plain query text. These options do not introduce Boolean,
phrase, proximity, wildcard, or other query-language syntax.
:::

:::mara decision ADR-OPT-IN-SEARCH-EXCERPTS
:mid: 01M1RY4EJSJK4NGDQKNSK5WVXH
:title: Keep excerpt inspection within search
:justifies: DES-SEARCH-EXCERPT-OPTIONS

Make excerpts opt-in search output with selected-item filtering rather than a
new preview operation. Compact default results avoid spending context on every
candidate's body, while callers can request matching evidence either during
discovery or after selection. Sharing search semantics avoids a second matcher
and preserves the caller's choice between an extra inspection call and a
larger initial response. Consecutive item reading remains the role of item get;
query-focused excerpts may omit intervening content.
:::
