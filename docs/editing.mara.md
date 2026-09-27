# Source mutation and explicit recovery

MID backfill and explicit rollback are the current write slice. Journal-producing
move/rename and other item/relation edits remain pending review.

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

Reject later manual edits or permission changes and preserve the journal. Reconcile targets to a recorded version before retrying. Preserve malformed/unsupported journals and restore trusted backups before removing them. Reads remain available. Concurrent manual filesystem edits are not coordinated by the advisory lock. Journal publication and automatic rollback of failed move/rename writes remain separate implementation obligations.
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
