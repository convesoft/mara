:::mara requirement REQ-ITEM-LIST
:mid: 01M3G050JRX3RP7XW31JW54JWY
:title: List compact items in canonical source order
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

CLI `item list` and MCP `item_list` return the same compact item-summary pages in document-path and source order. Exact flavour, custom-field, authored relation-name/alias and document/subtree filters narrow the result. Missing MIDs do not make a read mutate source or require semantic project validation.

Return bounded, complete continuations for unchanged sources/options; reject invalid, oversized or stale requests explicitly without silently skipping items. Full reads parse documents and discover code; any strict document or code-discovery failure fails listing. Code and adapter changes invalidate continuation as document/schema changes do.
:::

:::mara design DES-ITEM-LIST
:mid: 01M3G05R3KJ1GKM5573H2TTWKH
:title: Filter and page the composed corpus snapshot
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-LIST

Compose `load_documents` with `CodeIndex::load` in strict `load_corpus`; return the first code problem as an operation error, preserving baseline read behavior. Do not run semantic graph/field/MID validation as a listing prerequisite.

Each summary contains ID, optional MID, flavour, title, project-relative path and one-based line. Summaries use document-path/source order and have no body, ranking, excerpts or neighbours. Filters accept declared flavours, custom fields and relation names/inverse aliases. Values match exactly without trimming; repeated values within a field key and within each filter category use OR, while different keys/categories intersect. Relations match canonical declarations among authored outgoing item occurrences; do not resolve target existence to filter.

Paths match exact document paths or directory subtrees by components, including nested descendants but excluding similarly prefixed sibling directories. Reject empty, absolute, parent-containing and root-only values. Preserve baseline component normalization of leading/interior `.` and repeated/trailing separators; wildcard characters are not expanded. Omission selects all documents.

Return `{items, has_more, next_cursor}` with a null cursor exactly when no entries remain. Default limit is 20; accept 1–100. Limit the serialized domain result to 65,536 UTF-8 bytes including escaping and continuation; transport wrappers are outside that limit. Truncate titles at 256 Unicode scalars with `title_truncated: true`, appending `[title truncated]` in human output. Never truncate identity/location fields or silently omit an oversized item: fail with an actionable bounded error if it cannot fit alone. Human output finishes with a page continuation line.

Preserve the baseline versioned stateless cursor and list request discriminator. Its position binds the unchanged filters/limit, schema, application version, document bytes, discovered code bytes, accepted adapter/configuration bytes, and bytes/existence of explicitly authored file-only code endpoints, including binary and unconfigured files. Those endpoints use ordinary relative path components and remain confined to regular files inside the project. Reject malformed, stale, zero or out-of-range continuation positions and instruct restarting. This change-detection cursor is not an authentication token.
:::

:::mara verification VER-ITEM-LIST
:mid: 01M3G05VT8TT2CE0MTTYTMNZ65
:title: Verify complete filtered listing through CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-LIST
:verifies: DES-ITEM-LIST

Run `cargo test --locked --test item_list` against real CLI and stdio MCP processes in isolated temporary projects. Materialize documents directly; fixture creation does not require item mutation/backfill. Check compact deterministic source order, declared exact filters and relation aliases, directory component boundaries, complete count/byte pagination and title truncation. Check invalid declarations/paths/limits/cursors and oversized identities, preserving all source bytes and absent MIDs.

Compare CLI/MCP domain results and error behavior. Continue every returned cursor to exhaustion with unchanged options and no duplicate/skipped identities. Change document/schema/request, unmarked code, adapter query/configuration and explicit file-only endpoint bytes to require stale-cursor rejection; a missing adapter or malformed document must fail even on a fresh read. Verify CLI help and MCP tools/list expose the restored list surface. Run prior suites as regressions and record the tested revision. Search, get, related, semantic project validation and code navigation remain separate methods.
:::

:::mara evidence EVD-ITEM-LIST
:mid: 01M3G0FEJKRZFRTC8P7PZDTQ5Q
:title: Composed item listing passes real CLI and MCP workflows
:status: accepted
:result: passed
:captured_at: 2026-09-26T23:22:57Z
:subject_revision: cf08aed2f09226d270635f656c8b5fb8735f5bf9
:evidences: VER-ITEM-LIST
:evidences: VER-CODE-DISCOVERY
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS

The tested implementation was committed as `cf08aed2f09226d270635f656c8b5fb8735f5bf9`; Git reported a clean tree before this evidence was added. Linux x86_64, Rust 1.97.1, candidate `/tmp/mara72-target/debug/mara`; build settings `CARGO_TARGET_DIR=/tmp/mara72-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

Passed `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets`: 93 tests (2 units, 13 code discovery, 31 corpus, 10 item list, 17 bootstrap, 20 schema), none failed or ignored. Listing tests execute real CLI and stdio MCP processes with isolated Git/configuration and direct fixture documents. Compact source order, normalized directory boundaries, declared exact filters/aliases, absent-MID preservation, complete count/byte-limited pages, escaped Unicode titles, oversized errors and stale/invalid cursors pass. Unmarked code, query/configuration and binary file-only changes invalidate cursors; missing adapters and malformed source fail fresh reads. Outside symlink bytes do not influence cursors.

A separate read-only self-hosting comparison traversed all 56 then-current repository items in three pages of at most 25, comparing the candidate and installed baseline JSON exactly, including continuation tokens. Installed full-baseline MCP schema/project validation returned complete and valid with zero diagnostics. Selected intent passed one requirement; realization and verification each passed the requirement and design with all pages consumed. Those authoring checks do not claim candidate graph validation.

This is the restored item-list vertical slice composed from the separately verified document and code dependencies. Search, get, related, semantic project validation, code endpoint navigation, mutations and other baseline capabilities remain pending. The installed authoring binary was not replaced.
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-DISCOVERY
:mid: 01M1XSKPPMJAFMZK0WD8243M4P
:title: Discover canonical context outside item blocks
:status: accepted
:kind: functional
:derives_from: SCN-READ-DOCUMENT-CONTEXT

CLI `search` and MCP `search` discover matching items, sections, and ordinary Markdown blocks in one bounded surface, including narrative-only documents. Return explicit kinds, exact source locations, structural context and continuation. Narrative needs no authored identity or flavour. Do not duplicate item-body hits or add a separate document-search operation. Structure and reusable references follow [[DES-DOCUMENT-STRUCTURE]]; search selection follows [[DES-UNIFIED-KNOWLEDGE-DISCOVERY]].
:::

:::mara requirement REQ-ITEM-SEARCH
:mid: 01M1PXP2KGKART3Y9XWADR46F5
:title: Search items deterministically with scope filters
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Unified `search <text>` deterministically matches item ID, title, body, and metadata keys/values. Every distinct query term must match a complete word in the result's own searchable values, in any order and across fields. Scope filters and selected IDs are exact. Include fuzzy matches under [[REQ-FUZZY-ITEM-SEARCH]] and order results under [[REQ-SEARCH-RELEVANCE]]. Listing remains the separate compact operation under [[REQ-ITEM-LIST]].
:::

:::mara design DES-DETERMINISTIC-KEYWORD-SEARCH
:mid: 01M1PXP2KGHS11B9YGCD35EP8S
:title: Replaceable deterministic keyword matching
:status: accepted
:kind: behavior
:satisfies: REQ-ITEM-SEARCH
:satisfies: REQ-FUZZY-ITEM-SEARCH
:satisfies: REQ-SEARCH-RELEVANCE

Apply NFC normalization, full default Unicode case folding, NFC again, and Unicode word segmentation to queries and searchable values. Deduplicate query terms. Compare complete words with exact equality or the query-length limits in [[REQ-FUZZY-ITEM-SEARCH]], using `strsim` Damerau-Levenshtein distance. Retain originating fields for exact-only identity matching and the weights in [[REQ-SEARCH-RELEVANCE]]. Excerpt matching normalizes grapheme clusters while retaining their original byte ranges, preserving composition and case-fold expansion. This is a disposable projection over working sources; it writes no index or project data.
:::

:::mara requirement REQ-FUZZY-ITEM-SEARCH
:mid: 01M1RY3ME6RSKJ8DBXYN7VYTBX
:title: Recover word matches containing small spelling errors
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Search always combines exact and typo-tolerant whole-word matches without a mode flag. ID and MID field values permit exact normalized word matches only; titles, body, metadata keys and other values permit typo tolerance, including text that resembles a handle. Empty-term queries match all eligible results. After normalization under [[DES-DETERMINISTIC-KEYWORD-SEARCH]], query words of 1–3 Unicode scalars permit zero edits, 4–7 permit one, and 8 or more permit two. Edits include insertions, deletions, substitutions and adjacent swaps. Apply the same matching to excerpt occurrences. This is not stemming, synonyms, substring or subsequence search.
:::

:::mara requirement REQ-SEARCH-RELEVANCE
:mid: 01M1RY3MEY6HVP2Z8AFVE7YAH8
:title: Rank search results reproducibly by relevance
:status: accepted
:kind: functional
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE

Every distinct query term must match the result's own content. Results with exact matches for every term precede results needing typo tolerance. Within those groups, sum each term's highest matching field weight: ID/title/heading 3, body/other metadata 1. Headings inside an item contribute to that item. Repeated occurrences add no weight; parent titles, graph degree, node kind and document length add no bonus. Break ties by document path and source order, then apply bounds. Listing remains in source order.
:::

:::mara design DES-UNIFIED-KNOWLEDGE-DISCOVERY
:mid: 01M232SX5VJZ65J1ZGGKZ556S9
:title: Search items and Markdown blocks through one discovery surface
:status: accepted
:kind: behavior
:satisfies: REQ-DOCUMENT-CONTEXT-DISCOVERY

## Search selection

Top-level CLI `search` and MCP `search` return discovery format 2. `item search`, `item_search`, optional excerpt flags and node-kind filters remain removed. Select owning items, section headings and outermost ordinary Markdown blocks from the complete discovery graph under [[DES-DOCUMENT-STRUCTURE]]. Documents are not search hits. Descendants of items or blocks contribute to their owner; a section matches its heading, not descendant text. Whole large blocks remain one result.

Reuse exact field/flavour/path and canonical authored-relation filtering under [[DES-ITEM-LIST]]. Any ID/flavour/field/relation filter selects items only; path filters also permit narrative. Selected IDs/MIDs use OR and resolve exactly before intersecting other filters; missing/ambiguous IDs fail even if another filter would exclude them. Search relation filters accept `schema:` qualification; a name shared with built-in `contains` or `mentions` is ambiguous even when no conflicting edge is present. Search does not traverse built-in connections to find matches.

## Results and continuation

Use matching/ranking under [[DES-DETERMINISTIC-KEYWORD-SEARCH]] and [[REQ-SEARCH-RELEVANCE]]. Return `{format_version: 2, results: [{node, excerpt}], has_more, next_cursor}`. Nodes use the existing structural summary contract. Each hit includes one original-source excerpt of at most 240 Unicode scalars with byte/line ranges and a partial marker. Locate matches through normalized-word and decoded-heading source maps; never reconstruct displayed source. Item field excerpts conservatively report partial content. Without a field match, use a window from the owning source span.

Search retains the same 20-default/1–100 limit and 65,536-byte domain-result budget as listing. Preserve identity/location/context fields and fail on an indivisible oversized result. Filters and ordering precede pagination. Reuse source/schema/code/adapter/file-only invalidation from [[DES-ITEM-LIST]], with a distinct search discriminator, query and selected IDs. Reject malformed/stale/out-of-range cursors with a restart instruction. CLI and stdio MCP expose the same domain results. Complete node reads and direct-neighbour transports remain separately verified capabilities.
:::

:::mara verification VER-UNIFIED-SEARCH
:mid: 01M3H0MJBCNTCDT5FER1Q1AG0F
:title: Search canonical items and narrative through real CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-DOCUMENT-CONTEXT-DISCOVERY
:verifies: REQ-ITEM-SEARCH
:verifies: REQ-FUZZY-ITEM-SEARCH
:verifies: REQ-SEARCH-RELEVANCE
:verifies: DES-DETERMINISTIC-KEYWORD-SEARCH
:verifies: DES-UNIFIED-KNOWLEDGE-DISCOVERY
:verifies: DES-DOCUMENT-STRUCTURE

Run `cargo test --locked --test search --test discovery --test discovery_handles --test references` against the candidate. Use disposable project-owned fixtures and actual CLI/stdin MCP processes; named repository checks are explicitly read-only. Direct fixture creation may assign fixture identities; it does not prove product mutation/backfill.

Require mixed item/section/owning-block results without inherited parent terms or duplicate body hits. Verify exact-all ranking, per-term field weights and stable ties; Unicode normalization, edit-distance boundaries and exact-only identities; exact field/path/relation and ID/MID selection, including empty queries and vocabulary ambiguity. Consume bounded pages with matching CLI/MCP domain results; verify count/byte limits, original-source excerpts and decoded-heading offsets, oversized result errors, stale cursors and rejection of retired options.

Dependency checks preserve local graph scopes, exact UTF-8/CRLF/EOF spans, shared definitions, direct reference provenance/anchors, inert source contexts, summary bounds, deterministic handles across process restarts and edits to other documents. Keep source bytes unchanged. Validation-transport and real mutation assertions remain preserved for their respective checkpoints; these library checks do not establish those operations. Run all previously restored suites, formatting and Clippy before recording evidence.
:::

:::mara evidence EVD-UNIFIED-SEARCH
:mid: 01M3H10FSE3TNZBTCE1TXXJ53T
:title: Unified search checkpoint verification
:status: accepted
:result: passed
:captured_at: 2026-09-27T08:51:34Z
:subject_revision: bb389003cb16b743b9af3272f87f3df3dcc45bb9
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-ITEM-LIST
:evidences: VER-CODE-DISCOVERY
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS

Tested revision `bb389003cb16b743b9af3272f87f3df3dcc45bb9`; Git reported a clean tree before this evidence was added. Linux x86_64, Rust 1.97.1, candidate `/tmp/mara72-target/debug/mara`; build settings `CARGO_TARGET_DIR=/tmp/mara72-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

Passed `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets`: 133 tests (2 units, 13 code discovery, 31 corpus, 9 discovery, 4 handles, 10 item list, 17 bootstrap, 12 references, 20 schema, 15 search), none failed or ignored. Real CLI/stdin MCP fixtures cover mixed ownership, exact/fuzzy Unicode ranking, filters, original-source excerpts, complete bounded pages and stale cursors. The shared-filter regression rejects list continuation after selected IDs change. Fixture identities are created directly; mutation and validation-transport assertions remain pending. Dependency versions and sources remain within the preserved baseline lockfile.

A separate read-only real-repository comparison consumed every search page and matched candidate and installed-baseline JSON exactly, including excerpts and cursors: empty query 127 hits in 2 pages; `schema` 52, `validaton` 52 and `canonical source` 20 hits, each in 1 page. Installed full-baseline MCP schema/project validation completed with zero errors or warnings. Selected intent passed 4 requirements; realization and verification each passed 7 requirement/design roots, with all pages consumed. These authoring checks do not establish candidate project validation or trace matrices.

This checkpoint restores search and its reviewed document graph dependency. Complete get/related transports, code endpoint navigation, mutations, semantic validation and remaining baseline capabilities are still pending. The installed authoring executable was not replaced.
:::

:::mara requirement REQ-PARTIAL-ITEM-READ
:mid: 01M1RY3MDC8V9YE7744Z4CRY0Y
:title: Read large items in consecutive portions
:status: accepted
:derives_from: SCN-RETRIEVE-BOUNDED-KNOWLEDGE
:kind: functional

Top-level CLI/MCP `get` returns an item's complete parsed body when it fits, otherwise consecutive portions. Follow with ordered authored metadata, preserving repeated keys, empty values and complete values across fragments. Ranges and explicit continuation reconstruct original parsed values without gaps or duplication, including Unicode and oversized values. Do not enumerate neighbours. Preserve source bytes and identity. Bounds and fragments follow [[DES-BOUNDED-NODE-READ]].
:::

:::mara requirement REQ-DOCUMENT-CONTEXT-READ
:mid: 01M1XSKPPTZCGKHDSYW0SKMB6B
:title: Read discovered nodes through bounded Mara retrieval
:status: accepted
:derives_from: SCN-READ-DOCUMENT-CONTEXT
:kind: functional

Read an item, section, Markdown block or document by exact item ID/MID or discovery handle through CLI and MCP. Return kind, source location, structural context and consecutive content, with ordered metadata for items. Reconstruct complete oversized nodes; excerpts do not replace consecutive reads. Items return parsed body; other document nodes return exact source spans, including contained source. Use [[DES-DOCUMENT-STRUCTURE]] for handles/summaries and [[DES-BOUNDED-NODE-READ]] for paging. Direct connections remain separate.
:::

:::mara design DES-BOUNDED-NODE-READ
:mid: 01M3H16RZ59C4WZ7115PZ3105S
:title: Read source content and ordered metadata in consecutive pages
:status: accepted
:kind: interface
:satisfies: REQ-PARTIAL-ITEM-READ
:satisfies: REQ-DOCUMENT-CONTEXT-READ

CLI `get <reference> [--cursor TOKEN]` and MCP `get {reference,cursor?}` accept exact item IDs/MIDs, structural handles and code references under [[DES-CODE-READ]]. Return discovery format 2 with `node`, `content`, `content_range`, `metadata`, `metadata_range`, `has_more`, `next_cursor` and `format_version`. Reuse [[DES-DOCUMENT-STRUCTURE]] summaries/stale-handle errors. Non-items have empty metadata. No limit, neighbours, `item get` or `item_get`.

Serialized JSON domain results, including escaping and continuation, fit 65,536 UTF-8 bytes; transport wrappers are outside the budget. Fill whole remaining content first when possible, otherwise the largest fitting prefix on Unicode scalar boundaries. Then fill metadata in authored order. Fragments have `{index,key,value,range}`; text ranges have `{start_byte,end_byte,total_bytes,partial}` relative to the original body/value. Metadata ranges have `{start_index,end_index,total,partial}` and mark fragmented values partial. Preserve repeated keys, empty values and fixed identity/location/metadata-key fields. Fail with an actionable size error if headers or the next fragment cannot fit; never skip content or emit a non-advancing page.

Opaque cursors bind exact reference, content/entry/value positions and shared source/schema/code invalidation under [[DES-ITEM-LIST]], plus explicit file-only content. Repeat reference unchanged. Reject stale, malformed, initial, terminal, out-of-bounds, non-UTF-8-boundary and impossible-order positions with a restart instruction. ID and MID reads may have different cursors while returning equivalent content. Reads never write source or repair identity.
:::

:::mara verification VER-BOUNDED-NODE-READ
:mid: 01M3H17ZAZYA3KE2JZ8Q3TXH7B
:title: Reconstruct bounded node and code reads through CLI and MCP
:status: accepted
:method: test
:level: system
:verifies: REQ-PARTIAL-ITEM-READ
:verifies: REQ-DOCUMENT-CONTEXT-READ
:verifies: DES-BOUNDED-NODE-READ
:verifies: DES-CODE-READ

Run `cargo test --locked --test get` with disposable project-owned fixtures and real CLI/stdin MCP processes. Reconstruct complete content and ordered metadata from every page under the domain byte budget; include Unicode, escaped text, repeated keys, empty values, large headings and oversized identity failures. Verify exact lookup, stale handles/cursors, malformed positions, removed options and no neighbour expansion. Read items, sections, blocks, parent documents, file-only code and exact SCIP symbols; verify source preservation, modifier/wrapper content, missing/ambiguous/unsupported targets, binary rejection and file-only cursor invalidation. Restore deferred exact-identity and full-title reads from search tests. Run prior suites, formatting and Clippy. Inspect exact indexed symbol endpoints in real repository reads; keep installed authoring checks separate from candidate acceptance. These checks do not establish related transports, code relation evaluation, validation or mutation.
:::

:::mara evidence EVD-BOUNDED-NODE-READ
:mid: 01M3H1G0VMDXFBKE597MSSRTNR
:title: Bounded node and code read checkpoint verification
:status: accepted
:result: passed
:captured_at: 2026-09-27T09:00:00Z
:subject_revision: 34325b681c6df44946594545448bf124ac5de6f9
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-ITEM-LIST
:evidences: VER-CODE-DISCOVERY
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS

Tested revision `34325b681c6df44946594545448bf124ac5de6f9`; Git reported a clean tree before this evidence was added. Linux x86_64, Rust 1.97.1, candidate `/tmp/mara72-target/debug/mara`; build settings `CARGO_TARGET_DIR=/tmp/mara72-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

Passed `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets`: 143 tests (2 units, 13 code discovery, 31 corpus, 9 discovery, 4 handles, 10 get, 10 item list, 17 bootstrap, 12 references, 20 schema, 15 search), none failed or ignored. Seven restored get groups reconstruct item, section, block and document source through real CLI/stdin MCP pages, preserving Unicode, authored metadata order/repeated keys/empty values, complete fitting content and source bytes. Oversized identity, malformed/stale cursors, stale handles and removed interfaces fail as specified. Three code-read groups verify Rust/Python/JavaScript/TypeScript content including attributes/decorators/export wrappers, ignored file-only pagination and invalidation, binary rejection, missing/ambiguous/unsupported endpoints and project confinement. Search again verifies complete titles and exact-only get identity selection.

Read-only candidate/installed-baseline comparisons matched complete JSON for six real-repository references: `REQ-DOCUMENT-CONTEXT-READ`, the index section and parent document handles, `code:src/query/get.rs`, `code:src/query/get.rs::get`, and `code:GOAL.md`. Each fit one page; oversized consecutive reads are covered by fixtures. Installed full-baseline MCP schema/project validation completed with zero errors or warnings. Selected intent passed 2 requirements; realization and verification each passed 4 requirement/design roots, consuming all pages.

This restores bounded get and exact code resolution. Related transports, code-edge evaluation, mutation, semantic validation and remaining baseline capabilities are pending. Authoring validation/matrices use the installed full tool and do not establish candidate implementations of those operations. The installed executable was not replaced.
:::
