# Source mutation and explicit recovery

MID backfill, item creation/update/deletion/movement/rename, relation edits and
explicit rollback are restored. The shared journal publisher supports relation
edits, movement and rename.

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

Reject later manual edits or permission changes and preserve the journal. Reconcile targets to a recorded version before retrying. Preserve malformed/unsupported journals and restore trusted backups before removing them. Reads remain available. Concurrent manual filesystem edits are not coordinated by the advisory lock. The shared publisher is specified by [[DES-MUTATION-TRANSACTION]].
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

The shared journal publisher is used by relation mutations, item movement and item rename. Under the mutation lock, capture original bytes and permissions, stage every candidate, recheck operation-level project state and every preimage, then durably publish the format-1 journal defined by [[DES-MUTATION-RECOVERY]] before replacing any original. Sync staged files and, on Unix, affected parent directories; remove the journal only after all replacements succeed.

On an in-process publication failure, restore recorded originals and remove newly created destinations. If rollback encounters later manual edits or another failure, preserve recovery information and refuse further writers until explicit rollback succeeds. A stopped process leaves a journal for restart recovery. Recheck each preimage before replacement; the advisory lock does not coordinate manual filesystem edits. Multi-file publication is recoverable, not an atomic snapshot for concurrent readers.
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

:::mara evidence EVD-ITEM-DELETION
:mid: 01M3H5GQ7WBWE2MMY0705872S8
:title: Item deletion passes source and surviving-reference checks
:status: accepted
:result: passed
:captured_at: 2026-09-27T10:10:19Z
:subject_revision: 6b7f0540520ecf89a5b3e3f67a0782ff984b7b21
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
:evidences: VER-ITEM-DELETION

At `6b7f0540520ecf89a5b3e3f67a0782ff984b7b21`, formatting, `cargo clippy --locked --all-targets --all-features -- -D warnings` and `cargo test --locked --all-targets` passed. The unchanged-source full run passed 239 tests across 20 suites with no failures; the ignored interruption helper was explicitly run at three boundaries by its parent. The existing proc-macro-error2 future-compatibility notice remains. A temporary build quota was resolved by cleaning only generated Mara package artifacts and rebuilding.

Nine deletion groups cover CLI/MCP identity/result parity, exact source/separator/permission preservation, retained empty documents, failed lookup/empty list after deletion, all incoming reference locations, removed outgoing/self links and literal examples, invalid/incomplete corpus and request refusal, duplicate-heading/contained-anchor protection, inline demotion to blocking mentions, external outgoing assertions, active/pending locks and real Rust code-marker targets. Previous capabilities and single-file publication refusal regressions pass.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity without diagnostics. Selected intent passed one root; realization and verification each passed two roots with all pages consumed. CLI/MCP advertise and execute deletion. Validation transport checks use reviewed library source conformance where project/item transports remain pending. Move/rename, rule evaluation, matrices and remaining repository/tooling scope are not completed by this checkpoint.
:::

:::mara requirement REQ-ITEM-MOVEMENT
:mid: 01M1RKZY3F63775E1HM1N4T7QG
:title: Move an item without changing its identity or authored content
:status: accepted
:kind: functional
:derives_from: SCN-EDIT-CONNECTED-KNOWLEDGE

CLI `item move` and MCP `item_move` relocate exactly one item by exact MID or human ID within or between discovered documents. Destination and optional original one-based line follow creation's confinement/insertion rules. Require source conformance before and after movement.

Preserve MID, human ID, exact authored block bytes, metadata, body, line endings and typed relation endpoints. Surviving Markdown references, including incoming and carried links, must retain their destinations. Preserve unrelated bytes, other items and existing file permissions; retain empty source documents. Return identity and original/new path and opener line consistently across transports. Publish through the recoverable mutation transaction; do not rename, update content, delete the source file or commit to Git.
:::

:::mara design DES-ITEM-MOVEMENT
:mid: 01M1RKZY478CMMT2Q67Y1CA1GP
:title: Move source spans through a recoverable file transaction
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-MOVEMENT
:satisfies: REQ-RECOVERABLE-MUTATION

CLI accepts `item move REFERENCE FILE [--line LINE]`; MCP accepts `reference`, `file`, optional `line` and project selection. Return `{id,mid,old_location:{path,line},new_location:{path,line}}` using relative paths and one-based opener lines.

Under the mutation lock, load/validate source conformance and resolve exact identity. Require a confined, discoverable regular `*.mara.md` destination with an existing parent; a missing destination is allowed. Check an existing destination against the loaded corpus. Insertion coordinates refer to the original destination, including same-file moves. Reject item interiors; the moved item's start/end boundaries preserve content. Adjust later same-file positions by removed byte length; omission appends.

Transfer the parser's exact end-exclusive source slice through its closing line, removing only that slice. Preserve authored newline styles; add separators in the destination's first newline style (LF when empty), including a missing closing-line terminator when content follows. Do not coalesce existing surrounding blanks or remove the source document.

Reparse all candidates and apply reference correspondence without body-edit exemptions. Preserve carried relative links, incoming links to contained nodes and shifted heading/block targets. Require candidate source conformance, unchanged item count/identities/metadata/body/recognized mentions and the selected identity at its destination.

Use [[DES-MUTATION-TRANSACTION]] to stage all changed paths with recorded preimages and modes, recheck project/schema/corpus/discovery, publish the journal and replace files. Explicit recovery follows [[DES-MUTATION-RECOVERY]]. Source validation is independent of lifecycle/rule policies; journaled publication is recoverable rather than atomically visible across files.
:::

:::mara verification VER-ITEM-MOVEMENT
:mid: 01M3H5R4W8K353AWDWKYDE3QKQ
:title: Check identity-preserving moves and reference-safe publication
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-MOVEMENT
:verifies: DES-ITEM-MOVEMENT
:verifies: REQ-RECOVERABLE-MUTATION

Run `cargo test --locked --test item_movement` plus shared transaction failure/interruption regressions. Exercise cross-file and same-file CLI/MCP moves, ID/MID lookup and navigation after movement, original line coordinates, exact block/separator/newline/permission preservation, empty source/new destination and boundary moves.

Reject invalid/hidden/symlink destinations, item interiors, incomplete/invalid corpora, active/pending writers and Markdown contexts that hide or retarget content. Preserve incoming/carried links and structural destinations, including reference definitions; allow identity-only references across a move. Check typed-inline/external assertions and subsequent relation inspection. Run all previous suites, formatting/Clippy, canonical validation and selected traceability.
:::

:::mara evidence EVD-ITEM-MOVEMENT
:mid: 01M3H5YH9QBGDF68Z4ZBMWTBF9
:title: Journaled movement passes identity and reference preservation checks
:status: accepted
:result: passed
:captured_at: 2026-09-27T10:17:50Z
:subject_revision: 9c9069891b056efc8cb6be9327a2a32693ab24f3
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
:evidences: VER-ITEM-DELETION
:evidences: VER-ITEM-MOVEMENT

At `9c9069891b056efc8cb6be9327a2a32693ab24f3`, formatting, `cargo clippy --locked --all-targets --all-features -- -D warnings` and `cargo test --locked --all-targets` passed. The unchanged-source full run passed 250 tests across 21 suites with no failures; the ignored interruption helper was explicitly executed at three publication boundaries by its parent. The existing proc-macro-error2 future-compatibility notice remains.

Eleven movement groups cover CLI/MCP ID/MID parity, exact block transfer, permissions, retained source/new destination, original same-file line coordinates, missing final newline and boundary moves, safe destination/discovery/symlink refusal, read/navigation identity, incomplete corpus and active/pending writer refusal. Carried/incoming/definition links and shifted heading targets cannot silently retarget; identity/self links survive. Typed inline and external assertions preserve exact spelling and report their new source locations. Existing transaction tests cover preimage conflicts, automatic rollback, interrupted publication and explicit restart recovery; all prior suites pass.

Candidate CLI schema validation and installed-baseline MCP schema/project validation returned complete validity without diagnostics. Selected intent, realization and verification each passed two roots, consuming all pages. Both transports advertise and execute movement. Boundary movement preserves bytes/location but retains baseline publication rather than promising no file replacement. Rename, project/item validation transports, rule evaluation, matrices and remaining tooling/corpus scope are still pending.
:::

:::mara requirement REQ-ITEM-RENAME
:mid: 01M1RW50QWRZSKHK5TP4B4QMFV
:title: Rename human IDs without changing resolved identity
:status: accepted
:kind: functional
:derives_from: SCN-EDIT-CONNECTED-KNOWLEDGE

CLI `item rename` and MCP `item_rename` change exactly one human ID selected by exact ID or MID in a source-valid corpus. Require valid replacement syntax, the selected flavour's prefix and project-wide uniqueness. Preserve its MID, flavour and path; the new ID resolves to the original MID and the old ID no longer resolves.

Rewrite supported current-corpus human-ID relation and wiki-mention targets, including self-references and narrative. Preserve MID-authored targets, unrelated source, whitespace, line endings and permissions. Surviving relations/mentions must retain their MID endpoints and Markdown links their destinations. Code files stay read-only; refuse rename when a remaining human-ID code marker would break. Publish through recoverable replacement. An unchanged ID succeeds without writes or affected paths. Return MID, old/new ID and affected paths consistently across transports; create no alias, history record or Git commit.
:::

:::mara design DES-ITEM-RENAME
:mid: 01M1RW50RC4BK3RAH78AEJVR2T
:title: Patch parsed ID targets through the recoverable transaction
:status: accepted
:kind: interface
:satisfies: REQ-ITEM-RENAME
:satisfies: REQ-RECOVERABLE-MUTATION

CLI `item rename REFERENCE NEW_ID` and MCP `item_rename {reference,new_id,project?}` return `{mid,old_id,new_id,paths}` with unique changed relative document paths in lexical order. Under the mutation lock, require complete source conformance, exact identity and valid unique replacement ID. No-op returns empty paths without publication, but still requires valid source and the lock.

Plan patches from the selected opener, schema metadata relation values, typed inline assertions and parser-recognized item/narrative wiki mentions authored with the old human ID. Check exact opener/token/scalar preimages and reject overlap. Replace only ID token bytes, applying patches backwards per file. Preserve metadata value spacing, MID references, unsupported labelled syntax, literal/code/raw examples and all unrelated text.

Reparse candidates and apply source correspondence with the expected old/new ID substitution, preserving the target's MID. Do not grant a body-edit exemption. Protect unchanged Markdown links, including generated anchors affected by renamed mentions inside headings. Require complete candidate source conformance and unchanged item count, MIDs, expected IDs, flavours, paths and ordered relation/mention identities. External/code addresses remain authored strings; code comments are not rewritten, so human-ID markers can block rename while MID markers survive. Lifecycle/rule evaluation is separate from source conformance.

Publish all nonempty changes through [[DES-MUTATION-TRANSACTION]], even for one file. Recheck project/schema/corpus and each preimage before replacement; explicit recovery follows [[DES-MUTATION-RECOVERY]].
:::

:::mara decision ADR-RENAME-WITHOUT-ALIASES
:mid: 01M1RW50RVQY5MM2V2S7FQFCD1
:title: Keep one current human handle per durable identity
:status: accepted
:justifies: REQ-ITEM-RENAME

Replace the current human-readable handle without retaining aliases. The immutable MID supplies stable reference identity across human-ID changes. Persisted aliases would add naming state and ambiguous future ID reuse. Rewrite supported human references in the current corpus through the same recoverable transaction; external systems and historical revisions retain their authored text.
:::

:::mara verification VER-ITEM-RENAME
:mid: 01M3H666HKDNS4GS8RD33AR1QX
:title: Check human-ID rename, stable references and recovery
:status: accepted
:method: test
:level: system
:verifies: REQ-ITEM-RENAME
:verifies: DES-ITEM-RENAME
:verifies: REQ-RECOVERABLE-MUTATION

Run `cargo test --locked --test item_rename` and rename unit tests. Check CLI/MCP parity, old-ID absence/MID continuity, exact source/permissions, self/narrative/metadata/inline references, aliases/symmetry, unchanged MID/external/literal spellings, no-op and no Git commit. Reject invalid IDs/corpora, shifted heading destinations, human-ID code markers and active/pending writers without writes.

Retain one-file and multi-file injected failure/rollback, manual-edit conflicts, patch preimage checks and real rename-process interruption at every publication boundary followed by explicit recovery. Run all prior suites, formatting/Clippy, canonical validation and selected traceability.
:::

:::mara evidence EVD-ITEM-RENAME
:mid: 01M3H6FHB7YY0J25PF634MXCQ6
:title: Human-ID rename preserves identity, source and recovery
:status: accepted
:result: passed
:captured_at: 2026-09-27T10:27:14Z
:subject_revision: 666e9c2271583e01020d1a7636a4c82ec6a500eb
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
:evidences: VER-ITEM-DELETION
:evidences: VER-ITEM-MOVEMENT
:evidences: VER-ITEM-RENAME

At the subject revision, `cargo test --locked --all-targets` passed 262 tests across 22 suites with no failures. Two ignored subprocess helpers were each explicitly exercised at three interruption boundaries by their passing parent tests. Seven CLI/MCP rename groups cover byte/permission preservation, old-ID absence and stable MID navigation, narrative and typed references, inverse/symmetric/external assertions, heading-link protection, read-only code-marker behavior, no-op/writer gates and unchanged-source refusal. Five rename unit groups cover one-file publication, replacement failures, conflicting manual edits, process interruption/recovery and patch preimages.

`cargo fmt --all --check`, `cargo clippy --locked --all-targets -- -D warnings` and candidate CLI schema validation passed. Installed-baseline MCP schema/project validation returned complete validity with zero errors or warnings. Selected intent, realization and verification checks each passed both selected roots with all pages consumed. Test projects own their files, adapters, configuration and Git state; the read-only self-hosting test passed. These results cover restored capabilities only; project/item validation transports, rule/graph execution and other unreviewed baseline scope remain pending.
:::
