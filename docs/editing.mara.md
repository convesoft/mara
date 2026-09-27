# Source mutation and explicit recovery

MID backfill, item creation/update/deletion, relation edits and explicit rollback
are restored. The shared journal publisher supports relation edits; move/rename
remain pending review.

:::mara requirement REQ-RECOVERABLE-MUTATION
:mid: 01M1RKZY3VJKYP7V8GNDKR84GT
:title: Recover interrupted multi-file mutations without losing source data
:status: accepted
:kind: functional
:derives_from: SCN-RECOVER-INTERRUPTED-EDIT

Before replacing any original, movement and rename verify every planned edit against its preimage, stage every candidate and validate the complete candidate corpus. Journaled replacements record durable project-local recovery information first. An in-process replacement failure restores every original, including removal of a new destination; interruption supports explicit rollback.

Active, pending or unrecoverable transaction state blocks all further Mara content mutations. Recovery must not overwrite later manual edits. Failures identify the pending state and next recovery action. Reads and validation remain available against current files.
:::

:::mara design DES-MID-BACKFILL
:mid: 01M3H35NH6DHAPHV93FKDY8RSN
:title: Insert missing MID lines after complete source preflight
:status: accepted
:kind: behavior
:satisfies: REQ-MID-BACKFILL

CLI `project mid backfill` and MCP `project_mid_backfill` hold the project mutation lock and reject a pending journal. Load the recovering corpus and reject every source-conformance diagnostic except the typed missing-MID condition; authored message text cannot grant an exception. Preserve existing MIDs and generate unique canonical ULIDs for missing identities.

Insert each MID immediately after its opener, using the document's newline style. Preserve all other bytes and parse every candidate before writing. Replace each affected file atomically, retaining permissions. This baseline operation has no multi-file journal: earlier file replacements can remain if a later write fails. Repeating backfill inserts only identities still missing. Return `{project, changed:[{id,mid,path,line}]}` in path/source order, with absolute project root, relative document paths and resulting one-based MID lines.
:::

:::mara design DES-MUTATION-RECOVERY
:mid: 01M3H35T7MPZKPTFE19W2XTV9R
:title: Lock content writers and explicitly restore journaled preimages
:status: accepted
:kind: interface
:satisfies: REQ-RECOVERABLE-MUTATION

`.mara/mutation.lock` is a persistent advisory lock file. Its OS lock is released on operation end or process exit; never delete it to unlock a writer. Content writers reject an existing `.mara/transaction.json`. Both paths must stay inside the project without symlink traversal.

Journal format 1 is UTF-8 JSON with `format_version:1` and a non-empty `changes` array. Each entry has a unique normalized project-relative regular `*.mara.md` path, `before` (UTF-8 source or null for a new file), `after` (candidate source), and `mode` (null for a new file, otherwise `readonly` and optional `unix_mode`). Reject unknown fields, unsupported versions and malformed entries without changing files.

CLI `project transaction rollback` and MCP `project_transaction_rollback` take the exclusive lock and need resolvable project configuration, including an existing configured schema path, but do not read schema contents or require a valid corpus. Check all targets against their recorded preimage or candidate and permissions before restoring anything; stage all originals, recheck each target, restore existing files and remove new destinations. Compare readonly on all platforms and a supplied unix_mode on Unix. Sync staged files and, on Unix, affected parent directories. Remove the journal only after success. Recovery is retryable, including already-restored paths; no journal is a successful no-op. Return `{project,restored}` with absolute project root and relative restored paths.

Reject later manual edits or permission changes and preserve the journal. Reconcile targets to a recorded version before retrying. Preserve malformed/unsupported journals and restore trusted backups before removing them. Reads remain available. Concurrent manual filesystem edits are not coordinated by the advisory lock. The shared publisher is specified by [[DES-MUTATION-TRANSACTION]]; move/rename workflows remain separate implementation obligations.
:::

:::mara verification VER-MID-AND-RECOVERY
:mid: 01M3H35Y1K0NBS2820QYW4TWTN
:title: Check deliberate identity backfill and safe explicit rollback
:status: accepted
:method: test
:level: system
:verifies: REQ-MID-BACKFILL
:verifies: REQ-RECOVERABLE-MUTATION
:verifies: DES-MID-BACKFILL
:verifies: DES-MUTATION-RECOVERY

Run `cargo test --locked --test mid_recovery` and lock unit regressions. Use isolated projects with fixture-owned documents and published-format journal states. Verify real CLI/MCP backfill results, exact preservation of non-MID bytes and existing MIDs, correct lines/newlines/permissions, idempotence and preflight refusal, including authored text resembling a missing-MID message.

Check pending/active locks, rollback of existing and newly created files, schema-independent recovery, optional Unix modes, no-journal success and retryable partial restoration. Reject malformed/unsupported journals, manual edits, mismatched permissions and unsafe paths while preserving source/journal. Run prior suites, formatting and Clippy. Fixture journals verify recovery; they do not prove interrupted move/rename publication or in-process automatic rollback.
:::

:::mara evidence EVD-MID-AND-RECOVERY
:mid: 01M3H3J8JEF2N9JVY8BECJAF1R
:title: MID backfill and explicit rollback pass real transport checks
:status: accepted
:result: passed
:captured_at: 2026-09-27T09:36:14Z
:subject_revision: ad13ac689ad315aac5e7d536363e0ad6f84f23cb
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-CODE-DISCOVERY
:evidences: VER-ITEM-LIST
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-MID-AND-RECOVERY

At `ad13ac689ad315aac5e7d536363e0ad6f84f23cb`, formatting, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets` passed: 184 tests, none failed or ignored. The final full run used unchanged repository source.

Eight real CLI/MCP groups cover deliberate/idempotent backfill, typed preflight exclusions, unchanged non-MID bytes, result lines, LF/CRLF, existing identities/permissions, active and pending locks, schema-content-independent rollback, complete/already-restored journal states, optional Unix modes, malformed/unsafe journals, later manual edits and permission conflicts. Two lock unit regressions cover active locking and inherited-descriptor release. Existing capability suites pass. Recovery requires an existing configured schema path but does not read its contents. Portable journals preserve readonly; exact Unix permissions require recorded unix_mode.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity without diagnostics. Selected authoring-tool intent passed two roots; realization and verification each passed four roots, consuming all pages. CLI help and MCP tools/list expose both restored operations. Journal fixtures prove rollback handling, not journal publication, interruption or automatic rollback of move/rename. Those operations, project/item validation transports and rule/graph execution remain pending.
:::

:::mara design DES-MUTATION-TRANSACTION
:mid: 01M3H4H9HVG08YV6J7V5D8D9MW
:title: Publish staged source changes with durable rollback information
:status: accepted
:kind: interface
:satisfies: REQ-RECOVERABLE-MUTATION

The shared journal publisher is used by relation mutations. Under the mutation lock, capture original bytes and permissions, stage every candidate, recheck operation-level project state and every preimage, then durably publish the format-1 journal defined by [[DES-MUTATION-RECOVERY]] before replacing any original. Sync staged files and, on Unix, affected parent directories; remove the journal only after all replacements succeed.

On an in-process publication failure, restore recorded originals and remove newly created destinations. If rollback encounters later manual edits or another failure, preserve recovery information and refuse further writers until explicit rollback succeeds. A stopped process leaves a journal for restart recovery. Recheck each preimage before replacement; the advisory lock does not coordinate manual filesystem edits. Multi-file publication is recoverable, not an atomic snapshot for concurrent readers. Move/rename callers remain pending.
:::

:::mara requirement REQ-ITEM-UPDATE
:mid: 01M1RSQNH2J3Q3ZG1KHX3STH1Q
:title: Update item content without changing identity
:status: accepted
:kind: functional
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

CLI `item update` and MCP `item_update` partially update exactly one item by exact MID or human ID. Require a title, custom field replacement/clear or body replacement; omitted properties remain unchanged. Preserve identity, flavour, metadata relations, untouched source and permissions. Body replacement may intentionally change typed inline assertions.

Validate the complete candidate source corpus before one atomic file replacement. Permit only unchanged missing required bodies on existing scaffolds, reported as edit warnings; explicitly supplying an empty/blank required body fails. All other source-conformance errors block writing. Preserve surviving resolved references and refuse changes that hide items or retarget untouched links. Pending recovery blocks updates. Return the selected identity, relative path, actual changed fields and warnings consistently across CLI and MCP.
:::

:::mara design DES-ITEM-UPDATE
:mid: 01M1RSQNHERJ070G06PK18A77K
:title: Apply validated partial updates to source spans
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-UPDATE

CLI accepts `item update REFERENCE` with optional `--title`, repeated `--field KEY=VALUE`, repeated `--clear-field KEY` and `--body TEXT`; `--body -` reads stdin. MCP accepts the corresponding `reference`, `title`, `fields`, `clear_fields` and literal `body` (including `-`). Null title/body is omission.

Group fields by key in request order, replacing each complete sequence. Clear removes every occurrence of an optional field; clearing an absent field is a no-op. Reject structural/unknown/relation keys, set/clear conflicts, required-field removal and invalid scalars. Trim title/field values; distinguish an empty string value from removal. Reuse existing metadata slots and surrounding whitespace, remove surplus entries, append extra values after the last slot and new keys after metadata in lexical order. Leave semantically unchanged fields byte-identical. Preserve body boundaries and closing delimiter; normalize replacement body/new metadata to the document's first newline style and terminate nonempty bodies with a newline.

Require globally unambiguous identities and resolvable existing internal relations on the selected item before editing. The legacy selected-item lookup imposed this prerequisite even when body replacement could remove a broken relation. Parse the candidate, validate whole-corpus source conformance, and verify unchanged item count, identity, unrequested metadata and other items' bodies/mentions. Only typed missing-body diagnostics on unchanged, non-replaced scaffold bodies become warnings. This is source validation, not lifecycle/rule evaluation.

Reference correspondence protects surviving targets, including relocated usages, duplicate headings and anchored blocks. Explicit body replacement may remove/literalize occurrences or edit a reference definition to a valid new destination; other surviving active links remain protected. Metadata-only edits have no body-edit exemption.

Under the mutation lock, stage one candidate with original permissions. Recheck project/schema/corpus/discovery and original bytes/permissions before atomic replacement; no multi-file journal is published. Return `{id,mid,path,changed_fields,warnings}` with sorted unique changed keys and warning `{scope,path,line,message}` entries in resulting source coordinates. No effective change returns an empty changed list without replacement. Human output prints warnings to stderr.
:::

:::mara decision ADR-DRAFT-ITEM-UPDATES
:mid: 01M1RSQNHVC39H4Y9CCSTW1GQV
:title: Allow continued drafting with explicit edit warnings
:status: accepted
:justifies: REQ-ITEM-UPDATE

Permit title and field edits while an existing scaffold's required body is missing. A requirement for fully complete knowledge after every edit would prevent incremental drafting. Return unchanged missing-body diagnostics as edit warnings while corpus validation still reports errors. Explicitly replacing a required body with empty text and every other source-conformance defect remain errors.
:::

:::mara verification VER-ITEM-UPDATE
:mid: 01M3H4YREQW5Z46QVPMW218NJW
:title: Check partial edits, drafting warnings and surviving references
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-UPDATE
:verifies: DES-ITEM-UPDATE

Run `cargo test --locked --test item_update` and the single-file transaction regression. Exercise repeated/cleared/empty fields, title/body changes, stdin versus literal MCP body, CRLF, whitespace, permissions, unchanged adjacent documents/items and no-op publication. Check scaffold warnings/progression, typed diagnostic exemptions, invalid requests and ambiguous identity refusal through real transports.

Retain body-reference cases for explicit definition edits/literal contexts, relocated usages, intact section reordering, anchored paragraph replacement and duplicate/sibling target protection. Check typed inline relations and unchanged source on failure. Run the prior suite, formatting/Clippy, canonical validation and selected traceability.
:::

:::mara evidence EVD-ITEM-UPDATE
:mid: 01M3H54NJK5M5SCK9419R3MSAH
:title: Partial item updates pass source and reference preservation checks
:status: accepted
:result: passed
:captured_at: 2026-09-27T10:03:45Z
:subject_revision: fbc67b33d94547956e8c4958bb7bce9d3608afc3
:evidences: VER-PROJECT-INSPECTION
:evidences: VER-SCHEMA-DEFINITIONS
:evidences: VER-DOCUMENT-PARSING
:evidences: VER-CODE-DISCOVERY
:evidences: VER-ITEM-LIST
:evidences: VER-UNIFIED-SEARCH
:evidences: VER-BOUNDED-NODE-READ
:evidences: VER-DOCUMENT-NAVIGATION
:evidences: VER-RELATION-INSPECTION
:evidences: VER-CORPUS-CONFORMANCE
:evidences: VER-MID-AND-RECOVERY
:evidences: VER-ITEM-CREATION
:evidences: VER-RELATION-MUTATION
:evidences: VER-ITEM-UPDATE

At `fbc67b33d94547956e8c4958bb7bce9d3608afc3`, formatting, `cargo clippy --locked --all-targets --all-features -- -D warnings` and `cargo test --locked --all-targets` passed. The unchanged-source full run passed 230 tests across 19 suites with no failures; one ignored interruption helper was explicitly executed by its parent at three boundaries. The existing proc-macro-error2 future-compatibility notice remains.

Twenty-one update integration groups cover repeated/cleared/empty fields, exact metadata whitespace, CRLF, permissions, identity and adjacent-source preservation, stdin/literal MCP dash/null, no-op inode preservation, scaffold warnings/completion, invalid requests, typed diagnostic exclusions, bound MCP context, typed inline relation authoring and existing relation-resolution refusal. Thirteen of these groups retain distinct surviving-link, reference-definition, literal-context, section and anchored-block regressions through both transports. Single-file publication tests preserve source on verification/preimage failures. All prior suites pass.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity without diagnostics. Selected intent passed one root, and realization/verification each passed two roots, consuming every page. Both transports advertise and execute item update. Project/item validation transport assertions use reviewed library source conformance; those transports, remaining item mutations, rule evaluation and matrices remain pending.
:::

:::mara requirement REQ-ITEM-DELETION
:mid: 01M1RTTGFNW3Y7K8CQW16FJKYV
:title: Delete an item only when surviving references remain valid
:status: accepted
:kind: functional
:derives_from: SCN-EDIT-CONNECTED-KNOWLEDGE

CLI `item delete` and MCP `item_delete` remove exactly one item resolved by exact MID or human ID. Require complete source conformance before editing and in the surviving corpus. Refuse any surviving typed assertion, supported item/narrative mention, code marker or Markdown reference that would become invalid or change destination.

Report surviving item/narrative reference impacts with source paths, one-based lines and byte spans. References removed with the item, including self-references and outgoing assertions, do not block deletion. Remove only its block and minimally coalesce adjacent empty separators; preserve other source bytes, surviving items/references, line endings and permissions. Keep its document even if empty. Publish one atomic replacement under the mutation lock; pending recovery blocks deletion. Return the deleted identity and relative path without cascade, force, tombstone or Git operations.
:::

:::mara design DES-ITEM-DELETION
:mid: 01M1RTTGG2S7CSTJTG2ES0K9RM
:title: Preflight references before removing an item source span
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-DELETION

CLI `item delete REFERENCE` and MCP `item_delete {reference,project?}` use one operation and return `{id,mid,path}` with original project-relative path. Under the mutation lock, load a strict corpus, require source conformance and resolve one exact identity. This source gate includes missing required bodies and code-marker targets, but not lifecycle/rule evaluation.

Remove the complete parser item span, including a present closing-line terminator. When an empty LF/CRLF line precedes the span, remove at most the first following empty LF/CRLF line. Preserve whitespace-only lines, all other leading/trailing separators and surviving final-newline state; keep the file.

Project the candidate and apply surviving-reference correspondence without body-edit exemptions. Report every item/narrative impact ordered by path and byte offset, naming selected identity and each source item/relation or mention where applicable. Protect links to contained headings/blocks and other anchors shifted by removal. Then validate candidate source conformance, including code markers that would lose their target. Authors must resolve reported impacts explicitly.

Verify exactly one removed identity and every survivor's unchanged document path, complete source block and recognized mentions. Stage one file with original permissions and recheck project configuration, schema, corpus, discovery, preimage and permissions before atomic replacement. Reuse the single-file publisher without a multi-file journal. Manual filesystem edits are outside the advisory lock.
:::

:::mara verification VER-ITEM-DELETION
:mid: 01M3H5AHJX6GTADRXZGZJR3Y51
:title: Check safe item deletion and surviving-reference refusal
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-DELETION
:verifies: DES-ITEM-DELETION

Run `cargo test --locked --test item_deletion`. Exercise CLI/MCP delete by ID/MID, exact result parity, loss of lookup and retained empty file, separator boundaries, mixed newlines, permissions and unchanged surviving blocks/documents. Require all incoming reference locations and unchanged source on refusal; allow removed self/outgoing references and literal examples.

Cover duplicate heading retargeting, contained anchors, typed inline assertions and demoted mentions, external outgoing edges, code-marker targets, invalid/incomplete corpora and requests, active/pending locks. Retain the shared single-file preimage regression, run all prior suites, formatting/Clippy, canonical validation and selected traceability.
:::
