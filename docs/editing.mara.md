# Item movement and recovery

:::mara requirement REQ-ITEM-MOVEMENT
:mid: 01M1RKZY3F63775E1HM1N4T7QG
:title: Move an item without changing its identity or authored content

`item move <reference> <file>` and MCP `item_move` relocate exactly one item
resolved by exact MID or human ID. The destination and optional one-based
`--line` follow [[REQ-ITEM-CREATION]] and [[REQ-ITEM-INSERTION-SAFETY]]. The
complete project must validate before and after movement.

Movement preserves the MID, human ID, exact authored item block bytes, existing
line endings, metadata, body, mentions, and typed relation targets. It preserves
all bytes outside the removed block and destination separators, other items,
and existing file permissions. It leaves an emptied source document present.
Mara never commits to Git or deletes the source document.

Human output, CLI JSON, and MCP identify the same MID, human ID, old location,
and new location. Same-document movement is supported; bulk movement, renaming,
content update, and deletion are separate operations.
:::

:::mara requirement REQ-RECOVERABLE-MUTATION
:mid: 01M1RKZY3VJKYP7V8GNDKR84GT
:title: Recover interrupted multi-file movement without losing source data

Before replacing any original, movement verifies every preimage and insertion
point, stages every candidate, and validates the complete candidate corpus.
Cross-file replacement records durable project-local recovery information first.
An in-process replacement failure restores every original, including removing a
new destination. An interrupted operation supports explicit rollback.

Active, pending, or unrecoverable transaction state blocks all further Mara
content mutations. Recovery must not overwrite later manual edits. Failures
identify the pending state and the next recovery action. Reads and validation
remain available against the current files.
:::

:::mara design DES-ITEM-MOVEMENT
:mid: 01M1RKZY478CMMT2Q67Y1CA1GP
:title: Move source spans through a recoverable file transaction
:satisfies: REQ-ITEM-MOVEMENT
:satisfies: REQ-RECOVERABLE-MUTATION

## Movement

Use the parser's end-exclusive item source span: the opener through the closing
line, including its line terminator when present. Transfer that byte slice
without rendering or normalizing it; remove only that slice from the source.
A missing closing-line terminator is supplied as a separator when content
follows at the destination. New separators follow the destination's existing
newline style (LF when empty). Existing surrounding blank lines remain.

The insertion line refers to the original destination, including for same-file
moves. Reject points inside any item, including the moved item. Its start and
end boundaries in the same file are no-ops. Adjust later positions by the
removed span's byte length. Default positioning appends. Check the complete
candidate corpus and unchanged item identities, bodies, metadata, and references
to reject insertion into Markdown contexts that hide or reinterpret items.

Hold the project mutation lock, stage all candidates in their destination
parents, then recheck source bytes, permissions, discovery, project configuration,
schema, and the full corpus before publishing. Existing single-file creation,
relation mutation, and MID backfill take the same lock and reject pending
journals; their existing write semantics otherwise remain unchanged.

## Journal format 1

`.mara/mutation.lock` is a persistent advisory lock file. Its operating-system
lock is released on process exit; do not delete this file to unlock a writer.
It may be Git-ignored and is not product knowledge.

`.mara/transaction.json` is an immutable UTF-8 JSON recovery journal with
`format_version: 1` and a non-empty `changes` array. Each entry contains a
normalized project-relative `path`, `before` (original UTF-8 source or null for
a new file), `after` (candidate source), and `mode` (null for a new file,
otherwise `readonly` and optional `unix_mode`). Paths must be unique regular
`*.mara.md` files within the project, without symlink traversal. Unknown fields,
unsupported versions, or malformed entries prevent automatic recovery.

Sync every staged file, including preserved permissions. Publish the complete
journal atomically without overwriting an existing journal before replacing any
original. On Unix, sync its parent directory before replacements and each
document parent after replacement. Windows flushes staged files; directory
fsync is not available through this implementation. Individual replacements
are atomic; the full set is recoverable, not atomically visible to readers.
No external coordinator or Git operation participates.

Remove the journal only after every replacement succeeds. A remaining journal
always means rollback to the recorded preimages, even if all replacements had
finished before interruption. No automatic roll-forward is attempted.

## Explicit recovery

Stop other Mara writers, then run `mara project transaction rollback` (with
`--project` when needed), or MCP `project_transaction_rollback`. Recovery takes
the same exclusive lock and does not require a valid corpus or schema. It
checks every current target against its recorded preimage or candidate and
checks recorded permissions before restoring any file. Stage all originals,
restore existing files, remove destinations whose `before` was null, then
remove the journal. Recovery can be retried after interruption; no journal is
a successful no-op.

If a target differs from both recorded versions, preserve the manual edits and
journal, reconcile the file to its recorded preimage or candidate (including
permissions), and retry. If the journal is malformed or unsupported, preserve
it and restore affected files from a trusted backup before removing it. Never
remove a journal merely to unblock mutations. Abrupt process exit may leave
unpublished temporary files in destination directories; they are outside Mara
discovery and can be removed after recovery. Concurrent manual filesystem edits
during publication or recovery are not coordinated by the advisory lock.

## Results

Movement returns `{id, mid, old_location: {path, line}, new_location: {path, line}}`.
Paths are project-relative and lines are one-based item opener locations in
the corresponding original or resulting document. Rollback returns
`{project, restored}` with the absolute project root and project-relative paths;
`restored` is empty when no transaction is pending.
:::
