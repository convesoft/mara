# Baseline review

This review accounts for retained, changed, and removed capabilities. Pending
means the implementation and its distinct verification obligations still need
review; it does not authorize removing behavior. Product contracts live in the
linked capability documents rather than in this inventory.

The active candidate now supports project initialization, schema inspection and
definition validation, item creation/update/deletion/movement/rename/listing, unified search, bounded get, direct navigation, relation inspection/add/remove, MID backfill and explicit
journal rollback, project/item validation, current-state rules and structural
relation policies. Corpus conformance, document/code discovery
and the document graph are reviewed dependencies. Earlier full-runtime test rows
remain historical where their edit or validation transports
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
| Matrices, parameter binding and pagination | Retain the reviewed shared evaluator, request checks and bounded record projection; correct exact code-reference Markdown links. The matrix review below records the distinct tests and candidate checks. |
| Code endpoints, comment markers and language integrations | Verified SCIP identity and association checkpoint; see [[EVD-SCIP-CODE-HANDLING]]. Matrix integration follows the matrix review. |
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

## Code identity and association review

Selected capability: exact SCIP endpoints throughout code discovery, graph loading,
reads, relation inspection, item-authored mutations and validation. The contract is
[[DES-CODE-TRACEABILITY]]; [[VER-CODE-DISCOVERY]] owns acceptance. Matrix integration
is checked again with the matrix capability.

| Area | Review and disposition |
|---|---|
| Configuration and invocation | Use one language entry with command, extensions, optional encoding and paired grammar/query. Validate assets even for empty languages; invoke only for matching unignored source. Keep explicit incomplete errors for failed commands and malformed output. Commands inherit process permissions. |
| Identity | Retain standard SCIP protobuf parsing, global definitions, exact language/descriptor identity, package-independent spelling and collision refusal. Syntax declarations supply ranges only. Preserve literal backticks and canonical escaping. Shared declarations read as one enclosing interval. |
| Source and snapshot | Retain ordinary relative paths, canonical project confinement, ignore behavior and deterministic ordering. Hash unignored inputs for command stability and cursor identity. Exclude mutation lock, journal and `.mara-stage-` files; transaction staging must use that prefix before preflight reloads code. |
| Comment ownership | Retain query-captured comments, declaration name spans and modifier/wrapper content. Group adjacent captured comments through whitespace; retain individual marker spans, lexical boundaries, deepest owner and valid file fallback. Require one indexed global identity for a declaration-owned marker. |
| Integration | Keep the public disposable CodeIndex projection and existing graph/query/mutation boundaries. Preserve canonical edge deduplication, globally ordered occurrence inspection, missing-target completeness gating, source-preserving item edits and explicit recovery. |
| Verification | Retain discovery, confinement, marker grammar, content-span and transport regressions with explicit SCIP fixture identities. Replace syntax-name identity assertions with indexed identity assertions. Keep real-indexer fixture provenance; add genuine configured-indexer acceptance. Cover encoding, collisions, shared declarations, empty/populated transitions, strict failures and staging. |

The focused range regressions establish two required boundaries: marker ownership
matches the complete definition name span, and a supplied invalid SCIP enclosing
range is an error even when Tree-sitter supplies declaration content. The tests
first reproduced false-valid results, then passed after targeted corrections.

Completion requires formatting, Clippy, the relevant full suite, real CLI/MCP
navigation and mutation, exact marker endpoint inspection and candidate traceability
at the tested revision. Authoring-tool success is not candidate acceptance.

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
| Code dependency | Retain ordinary-path grammar and canonical project confinement. File-only reads accept local regular files without an adapter; references match exact SCIP identities with shared-declaration and modifier/wrapper content. Preserve missing-file, missing-symbol, ambiguous and unsupported failures. Binary file content is not text-readable. No graph evaluator is required. |
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

## Direct-neighbour checkpoint review

Selected increment: top-level CLI/MCP `related` for document nodes and code
endpoints. Complete the existing search → direct connection → get workflow,
with bounded pages, exact filters and source evidence. The dependency is the
read-only canonical relation projection plus external-address parsing, not
relation editing/inspection, rule evaluation or project validation.

| Area | Review and disposition |
|---|---|
| Semantic dependency | Retain item/code/external endpoint identity, inverse normalization, same-flavour and endpoint permission checks, MID-ordered symmetric pairs, and canonical-name/endpoint deduplication with occurrence counts. Restore only edge construction/resolution and the disposable graph; leave occurrence tokens, inspection and mutation helpers pending. |
| External/code endpoints | Retain exact local-only HTTP(S) addresses with no fabricated item identity or network reads. Code references reuse the verified resolver; marker and item-inverse assertions contribute to one edge. Binary file-only endpoints remain navigable although get rejects their content. |
| Navigation | Retain direct built-in connections and canonical schema edges. Namespace ambiguity is vocabulary-based. Alias filters do not reverse direction; flavour filters select item neighbours. Preserve selected unresolved-target errors and exact code-marker errors instead of presenting an incomplete selected traversal. |
| Ordering/self edges | Retain outgoing, incoming, then symmetric groups; deterministic source order and external-address order. Directed self-edges appear once outgoing by default or in the requested orientation. Symmetric self-edges appear once only in omitted/symmetric direction. Mentions retain parallel source occurrences. |
| Bounds | Retain 20-default/1–100 entry limits, 65,536-byte JSON budget, full mandatory summaries, and fingerprinted continuation after filtering/deduplication. Reject stale/invalid positions and oversized entries without skipping them. |
| Tests | Restore three navigation groups and five related retrieval groups: evidence/parent-child workflow, namespaces/retired interface, oversized root, complete ordered pages, stale requests, Unicode byte bounds, oversized neighbour and human rendering. Replace backfill fixture setup with fixture-owned identities; preserve original mutation/validation assertions in pending references. Add focused semantic alias/symmetric/self, external-terminal, code-marker and binary-file-only navigation groups from the broader baseline relationship workflow. |
| Contracts | Correct obsolete relation-occurrence pagination and double self-edge wording to the accepted format-2 semantic-edge contract. Preserve canonical relation design identity, but keep its mutation/inspection obligations pending. Scope current evidence to restored read operations. |

Completion requires real CLI/MCP navigation and complete pages, source
preservation, semantic endpoint/count checks, baseline parity, all prior tests,
formatting/Clippy, canonical validation, selected traceability and committed
exact-revision evidence. Existing historical navigation evidence does not prove
this corrected-bootstrap revision.

The navigation suite retains eight baseline read groups and adds four focused
semantic/failure groups. Direct fixture MID assignment replaces backfill setup;
source bytes are compared around read workflows. RelationGraph retains endpoint
identity and occurrence counts; its unused first-occurrence source field is
deferred until validation/inspection needs it, while parsed occurrence spans
remain intact. The only added direct dependency is the already-locked baseline
URL parser. No mutation, occurrence-inspection or policy evaluator is restored.

## Relation-occurrence inspection review

Selected increment: CLI `relation get` and MCP `relation_get`. Resolve an exact
canonical semantic edge and page its authored metadata, inline and code-comment
occurrences, preserving spelling and source spans. Reuse the reviewed resolver,
source loaders and bounded cursor helpers; no mutation/validation dependency.

| Area | Review and disposition |
|---|---|
| Identity and matching | Retain canonical/inverse, ID/MID, symmetric, code and external resolution. Inspect one semantic edge and preserve each author's identity, relation spelling and target. Missing edges return a structured error with zero count, not a fabricated empty success. |
| Tokens and bounds | Retain project/schema/corpus-bound occurrence selectors and request-bound continuation, 20-default/1–100 limits and the 65,536-byte domain budget. Full total count repeats on each page; oversized mandatory occurrences fail without skipping. Selectors are read evidence now; mutation consumption remains pending. |
| Ordering finding | The accepted interface requires source path/byte order. Baseline occurrence collection appends code comments after all item assertions, so a code path sorting before its item document can violate that order. Reproduce through real inspection before correcting ordering at the inspection boundary; preserve selector identity and mutation helper behavior. |
| Tests | Retain inspection obligations from mixed relationship tests: canonical/alias and symmetric equivalence, exact metadata/inline spans, code/item occurrences, external spelling, 25-occurrence continuation and stale requests. Materialize fixture sources directly; retain add/remove/rename/move/validation assertions for later checkpoints. Cover count/byte bounds, missing/invalid inputs and structured CLI/MCP errors. |
| Transport/knowledge | Restore only relation-get command/tool, parameters and result/error envelopes. Keep schema-edge navigation in its current design; inspection owns occurrence records/selectors. Reconcile the existing interface design identity without claiming unreviewed mutation operations. |

Completion requires real CLI/MCP inspection and source preservation, the ordering
regression, complete pages and stale/error cases, prior suites, formatting/Clippy,
canonical validation, selected traceability and exact-revision evidence.

The real CLI/MCP ordering regression failed on baseline collection with
`zzz.mara.md` metadata/inline entries before an `aaa.rs` code comment. Inspection
now sorts the collected records by path/start byte before applying page limits;
selector generation and mutation-facing collection remain unchanged. Five test
groups retain alias/symmetric/external source evidence, code ordering, complete
count/byte pages, stale/error envelopes and unchanged source. Schema staleness
checks alter a parsed declaration, matching the existing semantic fingerprint.

## Corpus conformance dependency review

Selected increment: the recovering corpus loader and source-level conformance
checks needed by validation and mutation preflight. Reuse reviewed document/code
loaders, discovery diagnostics and schema recovery. Project/item validation
transports, conditional rules, graph policies and mutation locks/writes remain
pending; this checkpoint must not expose incomplete project validation.

| Area | Review and disposition |
|---|---|
| Recovery | Compose the existing recovering DocumentSet with CodeIndex instead of duplicating discovery/parsing. Retain readable documents and independent diagnostics, propagate incomplete discovery, and associate code problems with human IDs and MIDs. Syntax-only recovery avoids code adapters when schema interpretation is unavailable. |
| Identity | Retain missing, malformed, misplaced and repeated MID checks; index every valid authored MID, including secondary entries; diagnose both sides of duplicate human IDs/MIDs. Do not rewrite identities. |
| Conformance | Retain independent flavour/prefix/body/typed-field checks, required/repeated fields, metadata and inline relation permissions, inverse/same-flavour endpoints, external syntax, and exact code resolution. Invalid declarations suppress only dependent checks. |
| References | Retain discovery diagnostics and ambiguity checks. Missing targets require complete item discovery; review the code-marker branch against this invariant, because the baseline checks missing marker targets unconditionally. |
| Diagnostics | Retain stable codes, authored spans and deterministic path/line/message order. Mutation-only missing-body/MID classifiers are deferred until their consuming operations; no message-based classification is introduced. |
| Tests | Restore four identity groups from pending tests at the library boundary. Retain recovery obligations for independent schema/item errors, title/body and metadata recovery, unreadable sources, incomplete target suppression, typed fields, item/inverse/external/code endpoints and source preservation. Existing transport assertions and mutation setup remain in the preserved baseline/pending tests for their own checkpoints. |

Completion requires isolated real-source library workflows, targeted reproduction
of any confirmed invariant violation, all existing CLI/MCP regressions,
formatting/Clippy, canonical validation, selected traceability and exact-revision
evidence. A passing dependency suite does not establish project validation or MID
backfill completion.

Fourteen library groups now cover the retained identity, recovery, field and
endpoint obligations. The recovered corpus composes existing loaders; the only
additional schema dependency is access to the recovery flags already populated
by definition validation. Existing strict reads and transports are unchanged.
The regression reproduced a false `reference_unresolved` code-marker diagnostic
beside an unreadable target document. The marker branch now applies the same
complete-discovery gate as item relations; complete missing and resolved target
counterparts pass. Code-problem association uses a malformed marker with a valid
target; a top-level comment after a struct is valid file-level attribution, not
an unsupported-owner fixture. No source rewrites or new dependencies were added.

## MID backfill and explicit recovery review

Selected increment: CLI/MCP MID backfill plus the rollback operation named by
its pending-journal failure. Dependencies are the reviewed corpus conformance,
ULID generator, per-file atomic replacement, and only the lock/journal-reading
portion of the transaction module. Move/rename journal publication, transaction
commit hooks, item creation/update and relation writes remain deferred.

| Area | Review and disposition |
|---|---|
| Backfill | Retain complete source-conformance preflight except typed missing-MID diagnostics; preserve all existing identities and bytes, insert after openers using existing newline style, and report resulting one-based lines in path/source order. Parse every candidate before the first write. Reads and repeated backfill remain no-ops. |
| Writes | Retain the baseline per-file atomic replacements and permission preservation. This operation does not create a multi-file journal or promise group rollback after a later I/O failure. Do not silently broaden its transaction semantics. |
| Locking | Retain persistent OS advisory lock with explicit unlock on drop, including inherited descriptor regression. Active writer/recovery and any pending journal block backfill before content changes. Lock/journal paths cannot traverse symlinks. |
| Recovery | Retain format-1 strict journal decoding, unique confined document paths, before/after and permission checks for every target before any restore, staged originals, removal of newly created destinations and journal removal only after success. Recheck each target during restoration; conflicts preserve source and journal. Recovery needs project configuration, not a valid schema/corpus. |
| Tests | Restore deliberate/idempotent and false-message preflight backfill regressions through real CLI/MCP, using library conformance until project_validate returns. Retain CRLF, result lines, existing MIDs and permissions. Restore rollback through both transports, optional Unix mode, malformed entries, permission/manual-edit refusal, active/inherited locks and idempotence. Journal fixtures represent published format-1 states; interruption/publication tests remain pending with move/rename. |

Completion requires these real CLI/MCP workflows, unchanged-source failures,
all prior suites, formatting/Clippy, canonical validation, selected matrices and
committed revision evidence. Neither fixture journals nor backfill establish
completion of the deferred journal-producing mutations.

Eight integration groups exercise both real transports; two restored unit
regressions cover active and inherited-descriptor lock release. Journal fixtures
cover fully published and already-restored states, optional Unix permissions,
manual/permission conflicts, malformed entries and symlink refusal. Recovery
requires resolvable project configuration: its configured schema path must exist,
but the schema contents may be unreadable and are not loaded. The recovery test
uses invalid UTF-8 schema contents to distinguish that boundary. The already-locked
`tempfile` dependency moves from dev-only to runtime for atomic replacement;
no dependency version changes. No production behavior correction was needed.
The portable-mode recovery case asserts readonly preservation; exact Unix
permission preservation is asserted when the journal records unix_mode. Omitting
that field does not carry enough information to reconstruct Unix mode bits.

## Item creation review

Selected increment: CLI/MCP item creation, including required-body scaffolds,
explicit line insertion and initial outgoing relations. Dependencies are the
existing corpus, conformance, relation projection and mutation lock/write helper;
add projected document replacements and creation-only reference preflight.

| Area | Review and disposition |
|---|---|
| Request/candidate | Retain scalar/field/prefix/uniqueness checks, generated MID, optional body/scaffold result, initial ID/MID/self/external edges, alias normalization and semantic duplicate rejection. Candidate conformance still rejects non-finite numbers even though the early numeric parser accepts them. |
| Source/discovery | Retain confined regular-file destinations with existing parents, content/ignore/symlink discovery checks, one-based insertion coordinates, blank separators, line endings and existing permissions. Parse the candidate, require exactly one new item and exact rendered body, then publish once. |
| Reference dependency | Retain character-diff correspondence by stable item identity or document narrative scope, structural target comparison and surviving occurrence matching. Preserve resolved targets when headings/anchors shift; validate newly authored references without requiring unrelated existing diagnostics to disappear. Restore only the creation entry point; rename-token and explicitly edited-body exemptions remain deferred. The baseline similar 3.2.0 text dependency provides this immediate correspondence requirement. |
| Tests | Retain complete/scaffold, inline/stdin body, fields, insertion/source preservation, hidden destinations, body escape, initial edge success/refusal/self/scaffold and unrelated-reference-error groups. Preserve mixed validation/update/rename/delete tests for their checkpoints. Exercise duplicate heading retargeting and valid unchanged targets through creation, plus CLI/MCP create-read-related parity. |

Completion requires real authoring workflows, candidate conformance, source/link
preservation, all prior suites, formatting/Clippy, canonical validation, selected
traceability and evidence for the committed revision. No relation editing,
item update/delete/move/rename, rule evaluator or project validation transport is
included.

Twelve creation groups retain complete/scaffold and optional-body behavior,
stdin versus MCP literal input, typed fields, safe line insertion, containment,
discovery confinement, real create/get/related parity, initial-edge refusal,
self/empty edges, heading-link protection, independent old/new reference errors
and writer blocking. Candidate conformance rejects non-finite numbers before
publication. The schema allows requirement-to-requirement derives_from; the
wrong-target fixture instead uses follows to an actual scenario. No production
behavior correction was needed. The reference helper omits rename and edited-body
parameters/exemptions; shared structural correspondence checks remain intact.

## Relation mutation and journal publication review

Selected increment: CLI/MCP relation add/remove, including whole-edge and
snapshot-selected removal. Multi-document inverse/symmetric assertions require
the shared journal publisher; existing recovery, corpus projection, source
correspondence, edge resolution and occurrence inspection are retained dependencies.

| Area | Review and disposition |
|---|---|
| Semantic writes | Retain canonical/alias endpoint resolution, global ID/MID uniqueness, semantic duplicate refusal, metadata insertion on the requested item and exact external spelling. Remove all item occurrences or one current selector; reject stale, mismatched and code-comment selectors. |
| Source safety | Retain reverse-offset edits, full metadata-line deletion and inline demotion to internal mention, external autolink or plain code reference. Reuse surviving-reference preflight without rename/body-edit exemptions; no policy gate on removal. Preserve unrelated source and permissions. |
| Transaction dependency | Restore permission capture, Change construction, staging, preimage/project rechecks, durable journal publication, per-file replacement and automatic rollback. Keep conflicts and recovery information when rollback cannot safely finish. Retain explicit recovery as the format owner. No move/rename or single-file commit helper is imported. |
| Tests | Retain inverse/symmetric/MID duplicates, selectors, whole-edge multi-file removal, inline/external/code demotion, wrong endpoints, source-preserving failures and heading-target regression through CLI/MCP. Retain publisher failure hooks before/after each replacement, preimage conflicts, incomplete rollback and actual subprocess interruption/restart. Existing journal decoding/permission and lock regressions remain active rather than duplicated. Mixed update/move/rename/delete and policy tests remain pending with those capabilities. |

Completion requires real transport workflows, source-preserving refusals,
automatic and restart recovery, all prior tests, formatting/Clippy, canonical
validation, selected traceability and evidence for the committed tested revision.

Eight relation integration groups pass through both CLI and MCP, including binary
file-only code targets, alias-named custom fields on ineligible flavours and
removal from an incomplete item. Four publisher unit groups retain injected
failures, preimage conflicts, incomplete automatic rollback and three real
subprocess interruption boundaries. The intentionally ignored child test is run
explicitly by its parent at each boundary. Existing lock tests stay active.
No production behavior correction or new dependency was needed; the inherited
reference preflight uses its existing two-argument entry point.

## Item update review

Selected increment: partial title/custom-field/body update through CLI and MCP.
Dependencies are existing strict corpus/conformance, metadata spans, query
identity resolution, source correspondence and mutation locking; restore the
single-file staging helper and explicit-body reference exemptions only.

| Area | Review and disposition |
|---|---|
| Partial edits | Retain request presence, scalar/type/repetition/required-field validation; replace grouped values, distinguish empty from clear, preserve unaffected slots/whitespace, identity, metadata relations and adjacent items. No-op requests do not replace files. Body replacement can intentionally change typed inline assertions. |
| Candidate safety | Retain whole-corpus source conformance, exact body recognition, item-count/identity/unrequested-metadata checks and typed unchanged-scaffold warnings. Do not exempt diagnostics by authored message text or gate on lifecycle/rule evaluation. |
| Reference dependency | Retain explicit reference-definition edits with newly resolved targets and literalization inside the replaced body; still protect other surviving occurrences, structural heading/block targets and external usages. Add only the edited-body parameter, leaving rename substitution deferred. |
| Publication | Retain same-directory staging, original permissions, project/schema/discovery/corpus and preimage rechecks, then one atomic replacement under the lock. This baseline path does not publish a multi-file journal. |
| Tests | Retain six update groups for repeated fields/source/permissions, stdin/optional body/no-op, scaffold progression, invalid requests and false diagnostic exemptions, CLI/MCP parity/bound context, and ambiguous identities/adjacent items. Retain the thirteen body-reference groups covering relocation, definitions, anchored edits/duplicates/siblings and literal contexts. Add typed-inline update coverage from the mixed mutation group; other move/rename/delete assertions stay pending. Restore the existing single-file verify/preimage refusal test. |

Completion requires real update/read workflows, exact source on success/refusal,
all previous suites, formatting/Clippy, canonical validation, selected matrices
and execution evidence for the committed tested revision.

The old update path used `get_item` only for identity but also resolved its
internal relation targets. Retain that precondition using the current shared
resolver; do not restore the legacy unbounded item result solely for lookup.
A transport regression covers refusal to repair an already-broken selected
relation via body replacement. The thirteen reference groups remain separate
because each distinguishes permitted target edits from a different retargeting
failure. Project/item validation transport assertions use existing library
source conformance until those transports are reviewed. Two additional groups
cover typed-inline body authoring and literal MCP dash/null/no-op behavior.

## Item deletion review

Selected increment: delete exactly one item through CLI/MCP while preserving
surviving source and references. Dependencies are already reviewed corpus
conformance, identity resolution, projected documents, reference correspondence,
mutation locking and single-file publication. No additional runtime is needed.

| Area | Review and disposition |
|---|---|
| Preconditions | Retain complete source-conformance validation before selection and after projection; incomplete scaffolds also block deletion. Use the existing exact ID/MID resolver after conformance instead of restoring the old retrieval result; relation targets are already validated. Lifecycle/rule evaluation is outside this source gate. |
| References | Retain all surviving metadata/inline assertions and item/narrative mentions plus Markdown heading/block destination protection, with ordered source locations for reference-preflight impacts. Removed outgoing/self references do not block deletion. Remaining code markers are protected by candidate conformance; do not edit code sources. |
| Source/publication | Remove the parser's complete item span and at most one following empty LF/CRLF line when joining empty separators. Keep all other bytes and the document itself, even empty. Verify exactly one removed identity and byte-identical surviving blocks/mentions, preserve permissions and recheck project/schema/discovery/corpus/preimage before atomic replacement. |
| Tests | Retain four deletion groups for source/separator/permissions/empty-file preservation, all-occurrence diagnostics with transport parity, outgoing/self/literal exemptions, and invalid corpus/request refusal. Extract deletion-only heading/anchor regressions from the mixed reference suite; retain typed-inline demotion/deletion blocking and external-only deletion obligations. Add real code-marker refusal coverage and writer blocking using existing helpers. Move/rename portions stay pending. |

Completion requires real delete/get/list outcomes, exact bytes on success and
refusal, all prior suites, formatting/Clippy, canonical validation, selected
traceability and execution evidence for the committed revision.

Nine focused deletion groups retain the four baseline groups and add bounded
checks for structural target protection, inline demotion to blocking mentions,
external outgoing assertions/whitespace-only lines, writer exclusion and real
Rust code-marker targets. Direct exact lookup is equivalent after full source
conformance, so no legacy read result was imported. No production behavior
change or new dependency was needed. The temporary build directory hit a quota;
cleaning only this package's generated artifacts allowed verification to resume.

## Item movement review

Selected increment: CLI/MCP movement within or between discovered documents.
Dependencies are existing destination/insertion helpers, strict corpus/source
conformance, source correspondence and the reviewed journal publisher/recovery.

| Area | Review and disposition |
|---|---|
| Identity/destination | Retain complete source validation, exact ID/MID selection, confined discoverable regular destinations with existing parents, optional new document and original one-based line coordinates. Use direct identity resolution after conformance rather than the legacy retrieval result. |
| Source | Retain exact parser block transfer, destination-style separators, same-document coordinate adjustment and unchanged-content boundary moves. Preserve empty source files and both existing modes. Reparse and verify item count, IDs/MIDs, metadata, bodies, recognized mentions and destination location. |
| References/publication | Retain incoming/carried Markdown destination checks, structural anchor correspondence and unchanged typed edges without body-edit exemptions. Recheck project/schema/corpus/discovery before journaled publication. Reuse tested interruption/rollback behavior without duplicating journal format ownership. |
| Tests | Retain six movement groups for cross-file bytes/permissions/graph identity, same-file original coordinates/new destinations, invalid destinations/context/corpus, CLI/MCP bound-context parity, missing final newline/boundary moves and symlinks. Extract carried/incoming/retargeted/definition-link and same-document heading regressions from the mixed suite, preserving removed-with-deletion tests in their existing checkpoint. Retain lock refusal and external/inline relation preservation; full mixed rename assertions remain pending. |

Completion requires actual CLI/MCP moves and subsequent reads/navigation, exact
source/identity on success and refusal, shared transaction regressions, all prior
suites, formatting/Clippy, canonical validation, selected matrices and execution
evidence for the committed revision.

Eleven movement groups retain the six baseline workflows and five focused groups
for carried/incoming/definition/shifted-heading links, identity/self links,
same-file boundaries, writer/incomplete-source refusal and inline/external edge
locations. The existing transaction tests still own publication failure and
restart recovery. No production behavior change or new dependency was needed;
exact lookup follows already-complete source validation. Boundary movement
preserves bytes/location; the retained publisher may still replace identical
content and does not promise the no-write behavior of item update.

## Item rename review

Selected increment: CLI/MCP human-ID rename preserving MID identity, supported
references and source. Dependencies are reviewed source conformance, patch spans,
identity resolution and journal publication; add only the rename-token mapping
to reference correspondence, retaining the explicit-body gate separately.

| Area | Review and disposition |
|---|---|
| Identity | Retain full source conformance, exact lookup, replacement grammar/prefix/uniqueness and unchanged-ID no-op. Resolve directly after validation instead of restoring the legacy retrieval result. Keep the baseline no-alias decision and stable MID. |
| Source patches | Retain opener, schema metadata, typed inline and parsed item/narrative mention target patches; check exact preimages and overlap, then apply in reverse byte order. Preserve MID spellings, prose, labels/literals, whitespace, newlines, paths and permissions. |
| Reference/publication | Allow expected human-ID token substitutions while comparing resolved destinations. Keep unchanged Markdown link/heading target protection and ordered MID endpoint checks. Every nonempty rename uses the journal, including one file; no-op still needs valid source and an available lock. Code files remain untouched: candidate validation rejects broken human-ID code markers, while MID markers survive. |
| Tests | Retain three CLI/MCP groups for exact bytes/MID graph/permissions/no Git commit, parity/no-op/bound context, and invalid request/corpus refusal. Retain five rename unit groups for one-file/no-op publication, every injected replacement failure, conflicting manual edits, real process interruption and patch preimages. Extract narrative/heading and inverse/symmetric/inline/external cases from mixed suites; retain code-marker and writer refusal boundaries. Shared rollback format tests remain active without duplication. |

Completion requires real rename/read/navigation through both transports, old-ID
absence and MID continuity, unchanged-source failures and interruption recovery,
all prior suites, formatting/Clippy, canonical validation, selected traceability
and evidence for the committed revision.

Seven integration groups and five unit groups exercise the reviewed rename
obligations. Keep process interruption in real subprocesses with fixture-owned
working directories and isolated Git/configuration. Preserve the baseline
read-only code-marker boundary and unchanged-ID no-write behavior, including
inode preservation. The heading-link fixture must begin its heading on a new
line; correcting that fixture required no production behavior change or new
dependency.

## Project and item validation review

Selected increment: real CLI/MCP project/item validation with complete diagnostic
summaries, reporting-only path selection and continuation. Required dependencies
are the reviewed recovering corpus and rule definitions, native rule evaluation,
and structural relation policies. Request-local checks, matrix observations,
parameter binding and matrix rendering remain deferred; no new dependency is needed.

| Area | Review and disposition |
|---|---|
| Validation orchestration | Retain independent configuration/source recovery, exact ID/MID item selection, source-owned diagnostics and full-target summaries before filtering/pagination. Hash discovered source, file-only code, adapters, accepted rule inputs and request options. Preserve existing schema-only behavior and classification fixes. |
| Policy prerequisites | Retain whole-corpus prerequisite gates even for item requests. Invalid definitions/source prevent apparent passes; retain actual blockers and explicit unavailability. Source mutation continues to use source conformance rather than lifecycle policies. |
| Native evaluation | Retain typed RDF projection keyed by MID, normalized distinct semantic edges, both directions of symmetric edges, and terminal opaque code/external identities. Keep class/path/applicability selection, upstream constraint semantics and the nested-error ledger. Preserve one deterministic reported violation per item/rule with authored locations, severity, optional native counts and generated message fallback. Defer matrix-only observation/cache export. |
| Graph policy | Retain eligible endpoint counts over canonical edges, distinct relation kinds, self-edge count once, and item focus filtering. Keep strongly connected components and deterministic authored cycle witnesses; external/code endpoints cannot close item cycles. No global acyclicity. |
| Source and reporting tests | Restore real transport checks for independent source/configuration recovery, item identity association and absence proof, unreadable files, diagnostic codes/locations, full-target hidden failures, pagination/snapshot invalidation, operation errors and byte bounds. Their library counterparts do not replace envelope/exit-status obligations. |
| Policy tests | Retain native nested-error regression; real lifecycle/qualified coverage, typed literals, nested endpoint classes, reserved names, same-flavour directions, finite depth/cycles, warning/error behavior, prerequisites, definition failures, authored messages and structural cardinality/cycle groups. Keep fixtures local, using standard Rust helpers and isolated child Git/configuration. Matrix-specific assertions remain with the deferred matrix checkpoint. |

Completion requires both transports on valid and broken disposable projects,
policy failure/repair, exact continuation and source preservation, all prior
suites, formatting/Clippy, canonical validation and selected traceability, then
evidence for the committed implementation revision.

Review outcomes: retain 52 source/transport groups, 17 policy groups and the
native nested-error regression. Existing schema-only tests own rejected
vocabulary, rule-source selection and schema envelope obligations; retain nested
field-type execution here to verify actual projected values. Three code-context
groups cover discovery failure, selected marker diagnostics/internal symlinks,
and rejection of partial results from invalid adapters. Fixture vocabulary/rules
live under tests/fixtures and runtime configuration is fixture-owned. Restore
only the small source-coordinate, item-completeness and first-edge-location
helpers needed by diagnostic projection. Keep no matrix-only cache export.

## Trace matrix review

Selected increment: CLI/MCP matrices over explicit root selections and either
persisted named rules or one request-local check, including exact-text revision
bindings. Reuse reviewed corpus recovery, item filters, semantic edges, rule
loading and native evaluation; restore only matrix projection/rendering,
observations and check binding. No new dependency or saved view format is needed.

| Area | Review and disposition |
|---|---|
| Selection and state | Retain normalized root filters or all:true, exclusive rule/check modes, expanded shape identities, rule applicability and full graph context outside root selection. A finite policy failure is matrix data; unavailable prerequisites retain issues and incomplete counts. |
| Request checks | Retain targetless named shape/type compatibility checks and exact text substitutions only in hasValue/in. Reject malformed/missing/unused/non-text bindings and duplicate CLI names. Bind in memory; configured policies still reject placeholders. |
| Explanation | Retain native observation/cache outcomes, source-linked reported obligations, immediate selected/qualifying counts, null unknowns, canonical edge context, per-evaluation summaries and bounded field inspection. Deduplicate authored edge occurrences without inventing item state on code/external endpoints. |
| Output and cursors | Retain record-stream pages, shared 65,536-byte JSON/Markdown budget, indivisible-record errors, snapshot-bound check references and unchanged-request continuation. Include code, file-only code, adapters and excluded invalid sources in snapshot identity. |
| Rendering finding | Baseline Markdown treats every non-external endpoint as an item, but code descriptors have only kind/reference. This renders a code edge as `[?](<>)`. Reproduce through real CLI/MCP Markdown before displaying its actual code reference and file link using the existing code-reference parser. Preserve external and item presentation. |
| Tests | Retain 13 matrix groups for rule states/inspection/parity, schema I/O errors, native literal counts, literal every semantics, external/code predicate states, source/adapter/excluded-source cursor invalidation, invalid prerequisites, second-hop continuation, request-check terminals and revision binding failures/repair. Restore the engineering acceptance/coverage/execution workflow. Add focused selection/byte-bound coverage and the observed code Markdown regression; keep disposable fixture-owned configuration and standard helpers. |

Completion requires real JSON/Markdown CLI/MCP parity and failures, complete
continuation, exact revision binding without source edits, full regression suite,
formatting/Clippy, canonical validation and candidate matrices over this corpus.


Matrix review outcomes: all sixteen matrix groups pass, including the genuine
engineering authoring/coverage/execution workflow. The code-link regression
first reproduced `[?](<>)` on both CLI and MCP, then passed with exact file and
SCIP descriptor labels, literal backticks and source links. Selection tests cover
intersection, empty valid results and rejected ambiguous requests. Byte-budget
tests consume every page without losing results and reject an indivisible
oversized identity. Retain the separate SCIP source-change validation-cursor
regression. Reuse the current item-filter API and preserve its established
selection behavior; retain existing native-engine failure tracking unchanged.

The combined checkpoint passes 366 tests across 26 suites, formatting and
Clippy with all targets/features. Two ignored subprocess helpers are exercised
by their parent recovery tests. Candidate corpus/matrix checks and exact-revision
evidence complete the checkpoint; the remaining interface, distribution and
public-guidance review remains separate.
