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
