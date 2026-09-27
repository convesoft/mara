# Baseline review

Reference: `0b47b42e31a9b1dedb71b1af5658a067c0a739a9` (`0.3.0-alpha.1`).
This review accounts for retained, changed, and removed capabilities. Pending
means the implementation and its distinct verification obligations still need
review; it does not authorize removing behavior. Product contracts live in the
linked capability documents rather than in this inventory.

The active candidate now supports project initialization, schema inspection and
definition validation, item listing, unified search and bounded get. Document/code discovery
and the document graph are reviewed dependencies. Earlier full-runtime test rows
remain historical where their edit, validation or navigation transports
have not yet been restored. The sections below identify current checkpoints.

| Baseline capability | Disposition and review boundary |
|---|---|
| Product intent and scenarios | Retain the two goals and eleven scenarios with their original identities in [product intent](product.mara.md). Add flows for connected editing, interrupted-edit recovery, and code associations from the existing baseline contracts. Remove obsolete release-relative wording from document-context discovery. No behavior change. |
| Canonical source, item identity, document format and schema | Retain source syntax, lossless parsing, item identity and deliberate MID backfill under [format contracts](format.mara.md). Restore all 30 corpus tests, 9 identity/backfill CLI tests and the Markdown-container unit test. Keep distinct Markdown-context, recovery, container/table-span, identity ambiguity and source-preservation obligations. The deterministic repository check is now explicitly read-only and independent of the historical document count. Identity diagnostics use stable codes, severity and source locations, with CLI/MCP parity; backfill also checks existing MIDs and exact preservation of other bytes. Full schema constraints, reference resolution and taxonomy review remain pending. |
| Project discovery, initialization and profiles | Retain initialization, project selection, schema inspection and flavour guidance under [project contracts](project.mara.md). Seventeen baseline tests retain distinct target-selection, transport, template, guidance, inspection and source-preservation obligations in `tests/project_bootstrap.rs`; add CLI/MCP parity for declaration inspection. Fixtures now own their Git/configuration state; guidance assertions use diagnostic codes/severity. Correct obsolete format-2 guidance prose to the implemented schema format 3. Fix the realization check with a requirement-class guard so design and mixed selections evaluate; a new real CLI/MCP regression first reproduced the baseline failure. Engineering policy, classification and execution semantics remain pending separate review. |
| Creation, update, move, rename, deletion and recovery | Pending: mutation contracts and reference preservation; CLI and mutation-reference tests. |
| Item and narrative discovery, search, navigation and bounded reads | Retain Markdown structure, references, source handles and direct navigation under [discovery contracts](discovery.mara.md), with 10 discovery, 5 handle, 13 reference and 3 real CLI/MCP navigation tests. Retain distinct heading-scope, anchor, source-span, process-restart, stale-handle, provenance, namespace and continuation obligations. Fixtures now own Git/configuration and child working directories; named self-host checks are read-only. Broken-reference diagnostics assert stable codes, severity and CLI/MCP parity. Handle and summary meaning now belongs to the structural design; the pending unified-retrieval design must link to it instead of repeating it. Full search ranking, filters, get/list/related continuation and response-bound suites remain pending. |
| Typed relations, aliases, symmetry, inline assertions and external targets | Pending: relation contracts; semantic-edge identity, occurrence inspection, mutation, and terminal external-target tests. |
| Validation, diagnostics and structural graph policies | Retain schema-only configuration recovery, format-1 diagnostics, output bounds and continuation under the schema definition review below. Project/item validation, reporting filters and graph evaluation remain pending. |
| YAML rules and native evaluation | Retain the reviewed definition loader, vocabulary/type checks, finite definition paths and native compilation for schema validation. Applicability, item projection, native evaluation and matrices remain pending. |
| Matrices, parameter binding and pagination | Pending: selected coverage, explanations, revision parameters, bounds, and matrix tests. |
| Code endpoints, comment markers and language adapters | Pending: endpoint resolution and navigation; real code checks and isolated multi-language fixtures. |
| CLI/MCP interfaces and project context | Retain project context and bootstrap transport/parity behavior. Full command help, tool schemas, and remaining operation parity tests are pending. |
| Packaging, installation, migration and releases | Pending: npm packaging and launchers, skill/plugin, migration, CI/release contracts, and real packaged CLI/MCP smoke. |
| Public guidance and historical research/release documents | Pending: README, roadmap, security guidance, generated changelog, and useful historical knowledge. Retained license notices remain unchanged. |

## Runtime and remaining test obligations

Commit `8205108` imported the complete baseline runtime before its capability
reviews. This was not the agreed incremental rebuild. The bootstrap correction
was committed as `7155e42`; previously recorded results
remain evidence for their exact revisions, not approval of the unreviewed runtime.
The installed baseline executable remains an authoring tool, not the candidate.

The dependency audit establishes a smaller entry boundary: CLI/MCP project
initialization and selection plus schema get/list need configuration, schema
declarations, template publication and transport dispatch. They do not load the
corpus or execute rules. The project type currently imports the code-language
configuration record from the code module; that data dependency does not require
the code scanner.

The earlier bootstrap tests cross that boundary. Schema validation loads YAML
rule declarations; project validation additionally loads source/code, evaluates
rules and graph policies, and paginates diagnostics. Creation and edits require
candidate-corpus validation, link-preservation preflight and transaction recovery.
Source/identity/navigation tests also exercise these operations. Preserving all
those workflows in the active build would retain unreviewed dependencies.

The user approved the narrow boundary on 2026-09-27. That correction checkpoint
contained five source files: project/schema decoding and template publication,
configuration diagnostics, operation wrappers, CLI dispatch, and stdio MCP.
Only `project init`, `schema get/list`, and their three MCP tools were advertised.
The full implementation remains preserved at `198a3aa` and in the original
baseline. No history is rewritten. Prior reviewed rows above record historical
work; source/identity/navigation/retrieval were inactive at that correction checkpoint.

### Bootstrap correction review

| Area | Decision and reason |
|---|---|
| Project selection and transport | Retain shared OperationContext, nearest-parent discovery and per-call loading. Absolute request paths and bound-server override rejection keep selection unambiguous. Narrow dispatch and help to supported operations; no stub endpoints. |
| Configuration and schema | Retain strict TOML/YAML decoding, formats and declaration sanity checks because inspection must reject malformed configuration. Keep code-language binding metadata as a plain configuration record; omit scanners, rule loading/evaluation and corpus validation. Declaration checks do not establish implemented graph policy. |
| Template publication | Retain embedded sources, complete preflight conflict checks, create-new writes and cleanup limited to files created by the attempt. Preserve the verified realization-template fix; executing that rule awaits its capability. |
| Runtime dependencies | Remove inactive corpus, Markdown, mutation, query, graph, code, rule and trace modules from the build. Retain nine direct runtime dependencies for parsing, matching project patterns and the two transports; tempfile is test-only. Lockfile pruning introduces no dependency version changes. |
| Initialization tests | Retain current/named/explicit targets, ambiguity, existing-content preservation, empty/minimal/engineering files and policy-file conflicts. These independently exercise selection and publication safeguards. |
| Context tests | Retain nearest/explicit selection, outside-project MCP startup, absolute request paths, unbound initialization and bound override rejection. Use schema_get instead of project_validate to observe selected context without importing its engine. |
| Inspection tests | Retain whole-schema, list and named declaration parity, configured schema path, unknown names, and distinct guidance type/content cases. Check malformed guidance through schema_get's operation-error contract; preserve structured validation diagnostic assertions for the later validation capability. |
| Boundary verification | Add CLI help and MCP tools/list checks so the advertised checkpoint matches executable operations. This is temporary rebuild verification, not a permanent reduction of product scope. |
| Deferred tests | Preserve the original bootstrap suite, corpus, discovery, handles, identity, references, navigation, retrieval work and helper source under `tests/pending/*.pending`. They do not compile or enter the source association graph. Restore each with its owning review; no obligation is dismissed. The Markdown-container unit test remains in committed source history. |

`VER-PROJECT-INSPECTION` defines the corrected narrow check. Existing evidence and
`VER-PROJECT-BOOTSTRAP` keep their original revision and broader method. Schema
validation, engineering authoring and realization execution remain unfulfilled
by the reduced candidate even where inspection has a requirement association.
Installed-baseline MCP validates the canonical corpus and selected traceability;
that authoring check is not evidence that the candidate implements validation.

Remaining baseline unit-test modules and integration tests remain pending restoration
with their owning capabilities, including isolated code-adapter fixtures. None
of their distinct obligations has been dismissed as redundant or obsolete.
The old cardinality code association is pending its contract review; new links
identify the reviewed initialization and schema paths. Missing coverage remains
visible in selected-scope matrices.

## Schema definition validation review

Selected increment: `schema validate` / `schema_validate`, including configured
YAML definition checking but excluding corpus conformance. Required dependencies
are existing configuration recovery, format-1 diagnostics and pagination, and the
baseline YAML binding/type checker plus pinned native SHACL compiler. No corpus,
code scanner, mutation, rule evaluation, discovery or matrix module is restored.

| Area | Decision and obligation |
|---|---|
| Configuration recovery | Retain independent project/schema diagnostics, available coordinates and null counts when the schema cannot load. Invalid prerequisites produce incomplete results; read failures remain operation errors. |
| Rule loading | Retain project-relative YAML/YML checks, canonical inside-project file checks, duplicate-source rejection, UTF-8 parsing and authored spans. Hash only accepted sources; rejected outside files must not influence cursors. Loading does not execute a rule or access authored network targets. |
| Binding/compiler | Retain supported-key rejection before JSON-LD, declared vocabulary/type/endpoint checks, named and anonymous definitions, consistent multi-file merges, cycle/depth checks and native compilation at registry 0.3.21. Omit the evaluation adapter and request-check/parameter APIs until their capabilities. Move only the existing root-path syntax predicate out of the absent query module; preserve its behavior. |
| Envelope/continuation | Retain common format-1 fields and stable codes as protocol data, deterministic ordering, whole-result summary, 1–100/default-20 count and 65,536-byte bounds, source/options cursor identity, and explicit oversized-record failure. Extract the schema-only branch without stubbing item/project validation. |
| Surfaces | Share the schema operation; retain CLI status/text/JSON and MCP successful invalid-result versus operation-error semantics. Add only schema_validate to MCP. |
| Test obligations | Restore schema declaration rejection and guidance cases; typed configuration/error envelopes; schema I/O failure; rule-source confinement and stale continuation; reusable-shape, endpoint and nested field-type compatibility. Keep valid definition counterparts. Retain mixed evaluation tests in the baseline for later restoration; declaration success never substitutes for item conformance. Add focused schema pagination/oversized-diagnostic and no-corpus-read checks. |

Completion requires these checks through real CLI and stdio MCP, canonical
knowledge validation, selected traceability, and exact-revision evidence.

The schema-specific interface and definition rules now have independent design
items in project contracts. The pending broader rule grammar and diagnostic
designs must link to them when restored, preserving their original identities
for remaining evaluation/projection meaning instead of duplicating these facts.
The native compiler and all transitive versions match the preserved checkpoint
lockfile; no dependency upgrade is part of this slice.

## Item-list dependency boundary

The next proposed vertical slice is CLI `item list` / MCP `item_list`: compact
item summaries, exact filters, deterministic order, bounded continuation and
source preservation. No implementation has been restored for this slice yet.

Baseline calls are `OperationContext::item_list` → `load_query_project` →
`load_corpus`, then `query::list_items` → `page::filtered_page`.
`load_corpus` parses selected documents and always calls `CodeIndex::load`;
code-discovery failures fail the read. The listing fingerprint includes document
bytes, discovered code bytes, configured adapter assets and file-only code
endpoint bytes. This is observable behavior, not merely an unused type import.

A real installed-baseline CLI check in a disposable, Git/configuration-isolated
project on 2026-09-27 established:

- Two valid requirements with one configured Rust adapter produced a one-item
  page and continuation.
- Changing only an unmarked Rust function made that continuation fail as stale.
- Removing only the grammar asset made a fresh item-list request fail; the
  documents remained unchanged.

Preserving the baseline therefore requires reviewing document discovery and
Markdown parsing plus code adapter loading/discovery/marker ownership and
snapshot identity before the item-list query/transport implementation. It does
not require mutations, graph validation, rule evaluation, navigation or matrices.
The smaller alternative is a dependency-only checkpoint before exposing listing;
omitting code discovery from listing would instead change compatibility and
requires an explicit product decision. Under GOAL.md's broad-import gate, settle
this dependency boundary before restoring these modules. Preserve the current
schema checkpoint and all pending tests meanwhile.

### Item-list review and distinct test obligations

| Reviewed boundary | Retain/change/defer decision |
|---|---|
| Compact result | Retain ID, optional MID, flavour, title, document path and one-based line in document-path/source order. The list path has no ranking, body, neighbours or excerpts. Restore only list-specific result and filter code, not the entire query module. |
| Exact selection | Retain declared-flavour/relation/field validation, relation-alias resolution, exact field values, OR within a filter category and AND across categories. Preserve directory-subtree component boundaries and exact-file selection. Keep the baseline retrieval path normalization; do not substitute stricter validation-reporting path syntax. |
| Read prerequisites | Retain strict document parsing and code-discovery failures. Do not add project validation as a precondition: the baseline list does not evaluate required fields, MID completeness or current-state policy. Parsing and semantic conformance are separate obligations. |
| Pagination | Retain default 20, limit 1–100, 65,536-byte serialized page limit, 256-scalar title truncation flag, deterministic continuation and explicit failure for indivisible oversized identity/location. Cursors cover source/schema/options plus the code inputs recorded above. |
| Required code dependency | Review adapter asset confinement, required captures, extension assignment, source walking, comment-only markers, ownership and snapshot inputs. Exact symbol resolution, relation projection, code navigation and backlink behavior remain separate unless a concrete list-path call requires them. Do not restore the whole code capability under this dependency label. |
| Preserved tests | Restore item-list branches of `directory_path_filters_preserve_boundaries_and_exact_files`, `bounded_search_and_list_continue_completely_with_cli_mcp_parity`, `item_list_and_search_return_deterministic_compact_filtered_summaries` and oversized-handle/invalid-page checks from `tests/pending/retrieval.rs.pending`. Preserve search, excerpt, get and related obligations for their own increments rather than deleting those branches from the reference. Add the demonstrated code-change/missing-adapter cases to listing tests if the coupled boundary is approved. |
| Fixture setup | Materialize source documents directly in test-owned projects; item creation is not a prerequisite for testing a read. Copy required grammar/query assets into those fixtures. Do not add synthetic marked source to the canonical corpus. |

This review makes the proposed restoration concrete; it does not resolve the
pending broad-dependency decision or authorize importing the modules. The
current candidate remains the verified project/schema checkpoint.

## Staged document dependency checkpoint

The user selected separate dependency checkpoints: document discovery/parsing,
then code discovery, then CLI/MCP item listing. This resolves the pending import
boundary above; preserve baseline listing behavior when composing those parts.
No listing endpoint is introduced by the document-only checkpoint.

| Area | Review and disposition |
|---|---|
| Document discovery | Retain configured glob matching, project-relative regular Mara files, no followed symlinks, local/parent Git-ignore handling, and stable path/source order. Strict loading fails rather than silently omitting an unreadable or malformed document. |
| Parser boundary | Retain the private Rushdown adapter and Mara-owned values. Code/raw/escaped contexts remain literal; delimiters, metadata and item bodies use original source offsets. Keep metadata order, exact bodies, mentions, typed relation occurrences and Markdown children without resolving a semantic graph. |
| Recovery | Retain independently recoverable item data and parse diagnostics, available source coordinates and completeness. Invalid title/metadata/structure suppresses unsupported body-block projection; no read repairs source or backfills identity. Semantic field/body/identity validation remains deferred. |
| Required implementation | Restore only document values, strict/recovering document loaders, filesystem selection and the three private Markdown adapter files. Name the entry points `load_documents` and `load_documents_for_validation`, returning a DocumentSet; do not expose a reduced `load_corpus` that silently skips its required code scan. Code discovery and full corpus composition follow in their own checkpoint. |
| Tests | Retain 29 corpus tests for discovery, parent/local ignores, deterministic self-hosting, syntax/recovery, literal contexts, UTF-8/CRLF spans, containers and table/reference spans; retain the Markdown-container unit test. Defer `title_recovery_preserves_independent_missing_body_diagnostics` because its distinct second assertion requires semantic corpus validation; preserve the full original suite in pending reference. Adapt loader names only, preserving assertions. Add two focused checks: unreadable UTF-8 retains other documents and reports unavailable coordinates without writes; document-only loading remains independent of unavailable code adapter assets, without claiming full listing can skip them. |
| Excluded implementation | Do not restore mutation replacement/preflight, identity indexes, semantic corpus/graph validation, code scanning, discovery navigation or query operations. The accepted source and format contracts and historical evidence remain intact. |

Completion checks: ordinary parser tests and the explicit read-only real-corpus
load, existing CLI/MCP regression suites, formatting and Clippy, installed-tool
knowledge validation and selected traceability, then exact-revision evidence.
This is the user-approved internal dependency checkpoint, not an item-list or
source-validation command release.

## Staged code-discovery dependency review

Selected capability: load local adapter assets and discover source files,
native symbol selectors, comment markers and their source spans. The user-approved
next dependency has no new CLI/MCP command. Listing will compose it with the
completed document loader; symbol resolution and graph operations remain pending.

| Area | Review and disposition |
|---|---|
| Adapter loading | Retain project-relative regular assets confined after canonicalization, UTF-8 queries, unique alphanumeric extension assignments, required symbol/name/comment captures and per-pattern name pairing. Build all adapters before walking; a bad pack yields a problem and no partial scan. Retain pinned Tree-sitter/Wasm versions; no network or project-code execution. |
| Discovery | Retain local/parent Git ignores, hidden supported files, deterministic path order, regular files and internal file symlinks, no directory-symlink traversal or outside-file reads. Document content patterns do not filter code. Read/walk problems remain explicit while independently readable files remain available. |
| Parsing and ownership | Retain native lexical qualification, computed-name exclusion, captured-comment-only marker parsing, full introducer and ID/MID grammar. Retain following declaration through modifiers/wrappers, deepest containing body, top-level file fallback, and explicit ambiguous/unsupported ownership. Keep source and content spans; semantic relation/target checks are not parser responsibilities. |
| Dependency boundary | Restore the scanner and its read-only snapshot values as a library dependency. Expose loading, files and accepted asset paths for direct verification and later composition. Defer exact-reference parsing/resolution, file-only endpoint bytes, graph summaries, corpus composition and cursor hashing until their owning reads; do not import discovery or query types. |
| Retained parser tests | Restore seven groups: nested selectors across four languages; block-comment markers; full introducer; body ownership boundary; unsupported nested/file fallback; modifier/wrapper attachment; modifier-inclusive content. Preserve the reference-path grammar test for later endpoint resolution. |
| Fixture correction | Replace parser unit tests' live-repository adapter lookup with ordinary integration tests whose temporary projects own Git/configuration state and copied packs. Keep Python/TypeScript assets under test fixtures only; do not enable extra adapters in the self-hosted project. Real tests have code associations; synthetic markers exist only as string data or temporary files. |
| Discovery test obligations | Add focused real-loader checks for ordering/ignore and content-pattern independence, malformed/unavailable packs and capture pairing, invalid marker and unreadable-source diagnostics, and asset/file symlink confinement. Preserve broader CLI validation/navigation tests in the baseline for their later capabilities. |

Completion requires real scanner execution with supplied Wasm grammars, unchanged
source bytes, existing candidate CLI/MCP regressions, formatting/Clippy, canonical
validation and scoped traceability, and committed exact-revision evidence. A
passing dependency suite does not establish code navigation or item listing.

Scanner implementation retains baseline behavior; the public read-only library
projection makes this dependency independently callable. Seven existing parser
test groups now run through the real loader; six focused discovery groups cover
file selection, bad packs, duplicate extension assignment, read/marker failures,
symlink confinement and walk failures. Original code contract MIDs are retained;
the method explicitly leaves endpoint/graph/transport verification pending.

## Composed item-list checkpoint

Both user-approved dependencies now have verified checkpoints. Apply the earlier
item-list review: restore strict corpus composition, list-only filters/summaries,
shared operation and CLI/MCP transports. Retain baseline request serialization,
stateless cursor format, page budgets and errors; omit search ranking, selected-ID
lookup, excerpts and relation navigation. Restore only ordinary code-reference
path parsing and confined file-only reads required by cursor fingerprinting;
exact symbol resolution is still deferred. Their distinct path-component unit
obligation and binary/outside-file behavior accompany the list checks.

The new independent list requirement/design own the retained listing portion of
the broader baseline retrieval contracts. When restoring search/get/related,
reconcile their shared pagination/path facts here instead of duplicating them.
CLI/MCP path guidance will describe actual baseline normalization: leading/interior
dot and repeated separators normalize, while empty/root/parent paths are rejected.
This corrects restrictive help text without changing accepted read behavior.

Completion: real filtered CLI/MCP pages through exhaustion, unchanged source and
MID absence, invalid/oversized requests, code/adapter/file-only cursor changes and
fresh-read adapter failure, existing suites and exact-revision evidence. Fixture
setup remains direct file creation; no mutation capability is imported for tests.

The list suite retains compact human-output, exact/path-filter, complete-page and
oversized-handle obligations from the pending retrieval suite. Its ten groups
add CLI/MCP parity throughout, byte-limited Unicode pages, invalid/stale positions,
code/query/configuration and binary file-only changes, outside-symlink exclusion,
and strict malformed-source failures. The original reference grammar unit test
is restored. Pending search/get/related tests and their assertions remain intact.

A read-only candidate/installed-baseline comparison traversed all 56 repository
items in three pages with identical JSON, including continuation tokens. No
listing behavior change was needed. The corrected path help describes behavior
already accepted and tested in the preserved baseline.

## Unified search checkpoint review

Selected increment: top-level CLI/MCP `search`, preserving discovery format 2,
item/section/outermost-block ownership, exact filters, ranked complete-word
matching, source-backed excerpts and bounded continuation. The existing strict
corpus loader already supplies document and code prerequisites. The remaining
call path is search → full document discovery graph, source summaries and
Unicode matching; no rule evaluation, mutation or code navigation is required.

| Area | Review and disposition |
|---|---|
| Structure dependency | Retain the three discovery modules: local heading scopes, original section/container spans, canonical item-edge occurrences, reference/anchor resolution and source-backed summary/handle indexing. The constructor always builds direct links; keep its complete library projection. Private graph indexes never escape. Existing structure contracts remain canonical. |
| Reference/handle failures | Retain source-only external/non-Mara links without network reads, percent decoding once, document-wide duplicate anchors, exact ID/MID resolution, ambiguity and stale-handle errors. Preserve current completeness gating on missing-reference diagnostics. Source reads never repair identities. |
| Search units | Retain owning items, section headings and outermost ordinary blocks; descendants of items/blocks contribute to their owner, document roots are excluded. Item-specific filters exclude narrative. Parent headings provide context without inherited matches. |
| Matching/ranking | Retain NFC/case-fold/NFC complete Unicode words, distinct-term AND, exact-only ID/MID fields, query-length edit budgets 0/1/2 and Damerau swaps. Exact-all matches lead; each term contributes its highest field weight, 3 for ID/title/heading and 1 otherwise. No repetition, node-kind or graph bonus; ties use source order. |
| Excerpts/bounds | Retain one default original-source excerpt per hit, 240-scalar window and exact byte/line mapping through normalization and decoded headings. Reuse existing count/byte/cursor helpers. Preserve single large nodes and explicit oversized-summary failure. Search has format 2; remove obsolete format-1/release-relative prose when restoring its contract. |
| Filtering/transport | Retain selected IDs resolved exactly before filtering, OR/AND field behavior and path normalization from listing. Search schema relations accept schema: qualification and reject ambiguous built-in names from vocabulary. Expose only top-level search; do not revive item_search or optional excerpt/kind controls. |
| Dependency tests | Restore nine independent discovery groups; keep actual create/update reload test pending. Restore direct link/anchor/span/inert-context tests; keep mutation and validation transport assertions pending, retaining their original tests. Restore source-handle stability/bounds checks with direct isolated fixture creation; preserve edit-command checks for mutation review. |
| Search tests | Retain distinct groups for mixed ownership/ranking, Unicode/exact identity matching, edit-distance boundaries, filters and selected IDs, directory/page composition, byte bounds, decoded-heading source offsets, removed options and stale cursors. Replace backfill-only fixture setup with fixture-owned identities; do not import editing to prepare read tests. |

Completion requires real CLI/stdin MCP parity and complete pages, matching source
spans, selected traceability and canonical validation, prior-suite regressions,
formatting/Clippy, and exact-revision evidence. Restored graph library coverage
must not be reported as completion of related/get or project validation.


The restored search suite contains 14 retained groups and one shared-filter
regression. Structure checks retain
9 discovery, 4 handle and 12 reference groups. Direct fixture creation replaces
create/backfill setup only. CLI create/update graph reload, mutation identity,
rename/delete preservation, full-title get and validation-transport assertions
remain unchanged in pending references; graph/span assertions from mixed tests
run now. No production behavior change was needed. The installed-baseline
comparison matched full JSON/cursors for empty, exact, misspelled and multi-term
queries over the real repository, consuming every page.

Restoring selected-ID state to the shared filters also requires restoring it in
the list fingerprint. The new library regression changes selected identities
between list pages and requires rejection; empty-ID CLI listing remains unchanged.
This preserves the baseline request-binding invariant during composition.

## Bounded get checkpoint review

Selected increment: CLI/MCP `get` for exact item IDs/MIDs, structural handles,
and explicit code references. Reuse the reviewed strict corpus, graph summaries,
source spans and fingerprint. Add only the get pager and exact code resolver;
code-edge evaluation, related transports and mutations remain separate.

| Area | Review and disposition |
|---|---|
| Selection | Retain exact graph lookup and stale-handle errors. Items return parsed body and ordered authored metadata; other document nodes return their complete source span. Do not infer neighbours or repair source. |
| Code dependency | Retain ordinary-path grammar and canonical project confinement. File-only reads accept local regular files without an adapter; selectors match exactly one indexed native symbol, including modifier/wrapper content. Preserve missing-file, missing-symbol, ambiguous and unsupported failures. Binary file content is not text-readable. No graph evaluator is required. |
| Paging | Retain discovery format 2 and the 65,536-byte serialized domain budget. Fill content before metadata; preserve repeated keys, empty values, entry order and UTF-8 boundaries. Fixed headers/keys cannot be dropped. Fail when a page cannot advance. |
| Continuation | Retain content/entry/value offsets, exact reference and source/schema/code fingerprint, including explicit file-only bytes. Reject malformed, stale, non-boundary, initial, terminal and impossible positions. Structural handles stay document-local while cursors cover the corpus. |
| Tests | Retain seven distinct get groups: item/human lookup failures, Unicode reconstruction, content priority, cursor rejection, oversized identity/neighbour exclusion, mixed-node source reconstruction and stale/removed-interface behavior. Restore full-title and exact-identity get assertions deferred from search. Add isolated code-file/symbol reads, failure classes and file-only cursor checks extracted from the broad baseline code workflow; keep its relation/mutation/validation assertions pending. |
| Contracts | Preserve requirement identities; replace retired item_get/limit/neighbour wording with current top-level get semantics. Keep read-specific fragmentation in one design and link shared summaries/handles instead of duplicating them. |

Completion requires complete source reconstruction through real CLI/MCP pages,
read-only source preservation, code resolution and failure checks, all prior
suites, formatting/Clippy, selected traceability, canonical validation and
evidence for the committed tested revision.

The get suite restores seven document-read groups and three code-read groups.
Four language fixtures own copied adapters; the candidate reads captured
modifiers and wrappers through CLI and MCP. File-only reads cover ignored text,
complete Unicode pages, binary rejection and changed-content cursor rejection.
Resolver failures cover missing files/symbols, duplicates, unsupported selectors
and paths outside the project. Search tests again read complete titles and
reject misspelled exact identities. Source and pending mutation/graph tests remain
intact; no production behavior change was needed.
