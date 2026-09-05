# Enhanced deterministic retrieval

:::mara design DES-RETRIEVAL-SCOPE
:mid: 01M1RYWDZXR3BVWW2V756KCPHK
:title: Define the alpha 3 retrieval boundary
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

This is the accepted scope for 0.1.0-alpha.3. Search/list pagination, search
excerpts, direct-neighbour pagination, consecutive partial item reads, and
typo-tolerant matching are implemented; relevance ranking remains planned.
Current retrieval contracts are [[REQ-ITEM-SEARCH]], [[REQ-ITEM-GET]], and
[[REQ-ITEM-RELATED]]. Open choices in the planned contracts must be settled
before implementation.

| Capability | Contract |
|---|---|
| Bounded search/list and opt-in excerpts | [[REQ-SEARCH-PAGINATION]], [[REQ-SEARCH-EXCERPTS]] |
| Bounded direct neighbours | [[REQ-RELATED-PAGINATION]] |
| Consecutive partial item reads | [[REQ-PARTIAL-ITEM-READ]] |
| Typo-tolerant word matching | [[REQ-FUZZY-ITEM-SEARCH]] |
| Deterministic relevance ranking | [[REQ-SEARCH-RELEVANCE]] |

Shared response bounds follow [[REQ-RETRIEVAL-BOUNDS]], and continuation follows
[[DES-RETRIEVAL-CONTINUATION]]. Each capability includes CLI/MCP parity and
focused verification through [[VER-BOUNDED-RETRIEVAL]]. Retrieval continues to serve
[[GOAL-BOUNDED-AGENT-CONTEXT]] while preserving [[REQ-CANONICAL-SOURCE]].

Traversal remains caller-controlled: inspect one item and its direct relations,
select relevant neighbours, and retrieve them as often as needed. No automatic
multi-hop expansion is added.

Semantic search (including lexical/semantic hybrids), embeddings or model
execution, synonym dictionaries, stemming, substring/prefix modes, a query
language, persisted indexes or graph stores, and context profiles remain
outside the implementation scope.

Narrative outside item blocks is canonical document content under
[[DES-DOCUMENT-FORMAT]], but current item search does not retrieve it. Whether
and how narrative should be searchable is an open alpha.3 investigation, not
an accepted implementation requirement. Evaluate its fit with Mara's purpose,
item identity, source locations, and bounded retrieval without assuming that
all useful narrative must become an item.
:::

:::mara requirement REQ-RETRIEVAL-BOUNDS
:mid: 01M1RY3MB07B3QJM0603QHMHBM
:title: Bound retrieval responses explicitly
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Search/list summaries, related-item summaries, and item retrieval must have
finite response bounds. Cap displayed summary titles and indicate truncation;
preserve complete item handles for subsequent retrieval. Full titles remain
retrievable through item get. Bounds must handle Unicode and oversized single
lines without presenting omitted content as complete.

Search/list and related pages default to 20 entries and accept `limit` from
1 through 100. Each serialized JSON domain result is at most 65,536 UTF-8 bytes,
including JSON escaping and pagination metadata; CLI framing and MCP transport wrappers
are outside that budget. The byte budget may shorten a page before its count
limit. Summary titles retain at most 256 Unicode scalar values and carry
`title_truncated: true` when shortened; human output appends `[title truncated]`.
Complete IDs, MIDs, flavours, paths, and relation names are preserved. If one
entry cannot fit an otherwise empty page, fail with a bounded diagnostic
directing the caller to shorten oversized identity/location fields or relation
names as applicable.
Never skip that entry silently.

Excerpt limits are defined in [[DES-SEARCH-EXCERPT-OPTIONS]]. Get uses the same
65,536-byte serialized JSON budget, including all fragments, ranges, and cursor
metadata. Its summary and neighbour titles use the same 256-scalar cap; retrieve
complete titles through the owning item's title metadata fragments. Identity,
location, metadata keys, and relation names remain complete; if a fixed header
or the next fragment/relation cannot fit an otherwise empty page, fail with a
bounded diagnostic directing the caller to shorten the oversized fields.
Body and metadata value fragmentation follow [[DES-RETRIEVAL-CONTINUATION]].
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
Continuation follows [[DES-RETRIEVAL-CONTINUATION]].
:::

:::mara requirement REQ-RELATED-PAGINATION
:mid: 01M1RY3MCMCZK45K50SS5TRYBX
:title: Continue bounded direct-neighbour results
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item related must return bounded pages of direct incoming and outgoing
neighbours, retaining relation names, directions, and existing filters. Make
remaining results explicit and retrievable through continuation. Preserve
outgoing-before-incoming and corpus/authored relation ordering. Neighbour
bodies require explicit retrieval by the caller. Paginate relation occurrences,
not unique neighbours: multiple relations to the same item retain separate
entries, and a self-relation retains its outgoing and incoming entries.
Apply filters before pagination; an empty result has no continuation.
Bounds follow [[REQ-RETRIEVAL-BOUNDS]] and continuation follows
[[DES-RETRIEVAL-CONTINUATION]].
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
Oversized titles and other metadata values must also be retrievable in
consecutive fragments. Preserve authored metadata order and repeated keys.
The continuation interface follows [[DES-RETRIEVAL-CONTINUATION]].
:::

:::mara requirement REQ-FUZZY-ITEM-SEARCH
:mid: 01M1RY3ME6RSKJ8DBXYN7VYTBX
:title: Recover word matches containing small spelling errors
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

CLI `item search` and MCP `item_search` always combine exact and typo-tolerant
word matches, without a mode flag or an exact-only fallback stage. Every
distinct query term must match at least one complete word in the existing
searchable values. Preserve all exact matches in the result set, including
when approximate matches exist; result sets may expand relative to exact-only
search. Empty-term queries retain their existing match-all behavior.

After the shared normalization in [[DES-DETERMINISTIC-KEYWORD-SEARCH]], allow
zero edits for query terms of 1-3 Unicode scalar values, one for 4-7, and two
for 8 or more. Use the query term's length, not byte length or the candidate's
length. Edits are insertions, deletions, substitutions, and adjacent letter
swaps. Apply the same matcher to excerpt occurrences.

Retain existing scope filters; item-handle lookup and every filter value remain
exact. This is word-level spelling tolerance, not file-picker-style subsequence
matching. Matching may incidentally cover a word-form variation or a nearby
word prefix; it does not promise morphological or substring matching.
Results retain corpus order until [[REQ-SEARCH-RELEVANCE]] is implemented;
that contract owns exact-before-approximate ranking before pagination.
:::

:::mara requirement REQ-SEARCH-RELEVANCE
:mid: 01M1RY3MEY6HVP2Z8AFVE7YAH8
:title: Rank search results reproducibly by relevance
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Item search must support relevance ordering that puts items matching every
query term exactly before any item requiring an approximate match. ID/title
weighting must preserve this precedence. Define scoring and field weights
before implementation. For unchanged input and options, ordering must
be reproducible with stable tie-breaking. Rank before limiting or paginating;
item list remains in corpus order. Whether relevance ordering is the search
default remains open, as does compatibility with current corpus ordering.
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
Without the excerpt option, return the compact summary shape, subject to
[[REQ-RETRIEVAL-BOUNDS]] and pagination metadata.
:::

:::mara design DES-SEARCH-EXCERPT-OPTIONS
:mid: 01M1RY4EGEC55K394TEZY5RYTR
:title: Expose excerpts through existing search operations
:satisfies: REQ-SEARCH-EXCERPTS

Extend `item search` and MCP `item_search` with equivalent options for excerpts
and exact selected-item filtering; do not add a separate preview operation.
CLI options are `--excerpts` and repeatable `--id <id-or-mid>`; MCP uses
`excerpts` (default false) and `ids` (default empty). Resolve every selected
handle exactly, rejecting missing or ambiguous handles even if other filters
would exclude them. Selections combine with OR, intersect the other filters,
and deduplicate ID/MID aliases before pagination. Selection order does not
change corpus order.

Only requested summaries include `excerpts`, an array of at most three
fragments in source order. Each fragment has `text`, `start_byte`, `end_byte`,
`start_line`, `end_line`, and `partial: true`; its summary supplies the file
path. Byte positions are document-relative, end-exclusive UTF-8 offsets;
lines are one-based and inclusive. Text is an exact source slice of at most
240 Unicode scalar values, including for oversized single lines.

Select occurrences using the same normalized exact/typo-tolerant word matcher
over ID, metadata keys/values (including title), and body. Take up to 60 scalar values
of preceding context within that searchable value, then up to 240 total;
skip occurrences already covered by a selected fragment. A match longer than
the fragment cap is itself clipped. Fragments may cross body lines, but never
searchable-value boundaries. Empty-term queries return an empty excerpt array.
Excerpts need not cover every query term or occurrence; `partial` always marks
them as an incomplete view.

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

:::mara design DES-RETRIEVAL-CONTINUATION
:mid: 01M1RYWE0PX1TCWXTTCYM35C7S
:title: Define continuation across bounded retrieval
:satisfies: REQ-SEARCH-PAGINATION
:satisfies: REQ-RELATED-PAGINATION
:satisfies: REQ-PARTIAL-ITEM-READ

Alpha.3 continuation lets callers retrieve omitted search results,
direct neighbours, body portions, and item relation entries. For unchanged
input, continuation must cover the requested content without omissions or
duplication. Rank search results before pagination; body reads remain
consecutive rather than selecting query matches.

Search/list and related accept CLI `--limit`/`--cursor` and MCP `limit`/`cursor`.
Their result is `{items, has_more, next_cursor}`; `next_cursor` is null exactly
when no results remain. Human output ends with a page line containing `has_more` and, when
present, `next_cursor`. Repeat the same operation, query, filters, limit, and
excerpt options with the returned opaque cursor. For related, repeat the exact
item handle, direction, relation/flavour filters, and limit. Related `items`
retain `{direction, relation, item}` entries, where `item` is a bounded neighbour
summary; both directions share one continuation sequence. Cursors are versioned,
stateless continuation markers, not persisted indexes or snapshot storage.

Reject malformed cursors and cursors whose source/schema or request fingerprint
has changed, with an instruction to restart from the first page. Any discovered
document change, including narrative edits, invalidates continuation. Reverting
to identical input restores validity; no historical snapshot is retained.
Cursor compatibility across application upgrades is not promised. Matching and
ordering finish before the count and serialized-byte budgets are applied.

## Item get

CLI `item get <id-or-mid>` accepts `--cursor` and `--limit`; MCP `item_get`
accepts `id`, `cursor`, and `limit`. Limit caps combined outgoing/incoming
relation occurrences per response: default 20, range 1 through 100. It does not
limit body bytes or metadata entries. The shared serialized-byte budget may
shorten any portion further.

One opaque cursor advances a single sequence: consecutive body bytes, ordered
metadata values, then outgoing and incoming relation occurrences in their
existing order. Fill the available byte budget in this order; return the whole
body on the first page when it fits alongside the fixed header and progress
metadata. There are no independent section cursors. Completed sections return
empty portions on later pages; repeat the same handle and limit until
`has_more` is false. Get uses the same source/schema/request change rejection
as other retrieval operations, including edits outside the selected item.

The JSON result retains `summary`, `source`, `body`, `metadata`,
`outgoing_relations`, and `incoming_relations`, and adds `body_range`,
`metadata_range`, `outgoing_relations_range`, `incoming_relations_range`,
`has_more`, and `next_cursor`. `summary` is bounded; `source` continues to locate
the complete item in its document. `next_cursor` is null exactly when no
content remains. Relation entries retain `{relation, item}` summaries.

- `body` is an exact consecutive slice of the parsed body. `body_range` is
  `{start_byte, end_byte, total_bytes, partial}` with end-exclusive UTF-8 byte
  offsets relative to the body. Split only at Unicode scalar boundaries.
- Each metadata fragment is `{index, key, value, range}`. `index` is the
  zero-based authored metadata occurrence, including structural metadata.
  `value` is a consecutive slice of that parsed scalar; `range` has the same
  shape as `body_range`, with offsets relative to that metadata value. Preserve
  complete keys and distinguish repeated keys by index. A title is retrieved
  completely by concatenating its metadata value fragments.
- Each collection range is `{start_index, end_index, total, partial}` with
  zero-based, end-exclusive indices. Metadata ranges cover touched entries;
  consecutive pages can touch the same index when its value is fragmented.
  Relation ranges index their respective direction independently. Empty portions
  report equal start/end positions, and total always describes the full section.
- `partial` is false only when that portion represents the entire value or
  collection. A metadata collection is partial if any value is fragmented.
  Empty completed sections of a nonempty value/collection remain partial;
  `has_more` describes continuation of the whole item, not completeness of each
  section shown on this page.

Human output labels every fragment's index/byte range and each section's
range, totals, and partial state, then ends with the same page continuation
line as other retrieval operations. Display separators are not body/value
content; reconstruct exact text from JSON strings and their offsets.
:::

:::mara verification VER-BOUNDED-RETRIEVAL
:mid: 01M1RYWE2BFJ41BZ9R4A3N5RNF
:title: Verify the complete bounded retrieval workflow
:depends_on: DES-RETRIEVAL-SCOPE
:depends_on: DES-RETRIEVAL-CONTINUATION

For alpha.3, exercise the real CLI and MCP against the same corpus: search,
continue results, optionally request excerpts for selected items, read an item
and its remaining portions, inspect direct relations, and fetch caller-selected
neighbours. Equivalent inputs must yield the same domain results.

Use large titles, large relation sets, Unicode text, and an enormous single-line
body to verify explicit bounds and complete continuation. Verify that excerpts
appear only when requested and identify matching source passages; consecutive
reads must retain text between those passages. Use real queries with expected
items to demonstrate typo recovery, useful relevance ordering, and preservation
of exact matches. Repeat unchanged queries to verify stable ordering and page
coverage. For related pages, verify both directions across page boundaries,
filtered continuation, title truncation, the serialized byte budget, and
rejection after source, schema, or request changes. For get, reconstruct the
body and all metadata values by their ranges, including enormous titles,
repeated keys, empty values, and JSON-escaped Unicode. Verify get's combined
relation limit and direction order, byte-budget continuation, cursor rejection,
and CLI/MCP domain-result parity through the final page.

Release preparation follows evidence that this workflow passes; these are
planned checks, not evidence that alpha.3 functionality already exists.
:::

:::mara decision ADR-SEARCH-CONTINUATION
:mid: 01M1S1WPMD2PNHTZ4643CCB1CN
:title: Restart retrieval continuation after source changes
:justifies: DES-RETRIEVAL-CONTINUATION

Use stateless, versioned search/list, related, and get cursors that bind result
positions to the source and request fingerprint. Reject changed inputs rather
than silently continuing an offset into a different result set. This preserves complete page
coverage for unchanged input without storing snapshots or an index; callers
restart discovery after edits.
:::

:::mara decision ADR-FRAGMENT-ITEM-READS
:mid: 01M1S5WD583NZJP58HSRZNFNMB
:title: Continue body and metadata fragments through one item cursor
:justifies: DES-RETRIEVAL-CONTINUATION

Use one continuation sequence for body text, metadata values, and direct
relations, with the body first. Fragment oversized title and metadata values
instead of rejecting otherwise readable items; this keeps complete authored
values retrievable within the response budget. A shared cursor gives callers
one completion condition and avoids coordinating independent section reads.
Fixed identity/location fields, metadata keys, and relation names remain intact;
report an actionable size error when they prevent progress.
:::

:::mara decision ADR-AUTOMATIC-TYPO-TOLERANCE
:mid: 01M1S7ES0HDMSMX4TWEMW9MM8Z
:title: Combine exact and typo-tolerant search automatically
:justifies: REQ-FUZZY-ITEM-SEARCH

Use one search operation that always combines exact and conservative approximate
word matches. Users and agents should recover spelling errors without detecting
a failed search and retrying in another mode. Preserve every exact match while
allowing additional candidates; requiring every query term and restricting edits
for short terms limits unwanted matches. Relevance ordering owns
exact-before-approximate precedence under [[REQ-SEARCH-RELEVANCE]]; this matching
change retains corpus order until ranking is implemented.
:::
