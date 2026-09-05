# Item editing and recovery

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
checks recorded permissions before restoring any file: compare `readonly` on
all platforms, and compare `unix_mode` only on Unix when the journal supplies
it. Stage all originals, restore existing files, remove destinations whose
`before` was null, then remove the journal. Recovery can be retried after
interruption; no journal is a successful no-op.

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

:::mara requirement REQ-ITEM-UPDATE
:mid: 01M1RSQNH2J3Q3ZG1KHX3STH1Q
:title: Update item content without changing identity
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY

`item update <reference>` and MCP `item_update` partially update exactly one
item resolved by exact MID or human ID. Require at least one requested change:
replace the title, replace all values of named custom fields, clear optional
custom fields, or replace the body. Omitted properties remain unchanged.

Validate title, field types, repetition, requiredness, body structure, and the
resulting project before writing. The only allowed remaining validation errors
are unchanged missing required bodies on existing scaffolds; return each as an
edit warning. Explicitly replacing a required body with empty or whitespace-only
text fails, including on a scaffold. `item validate` and `project validate`
continue to report missing required bodies as errors.

Preserve MID, human ID, flavour, typed relations, unaffected metadata order and
whitespace, untouched body Markdown, existing line endings, file permissions,
and every byte outside the selected item. Reject invalid requests without
changing source. Validate the candidate document and project, then publish one
atomic file replacement under the project mutation lock. Pending recovery state
blocks updates as described in [[REQ-RECOVERABLE-MUTATION]].

Human output, CLI JSON, and MCP identify the same MID, human ID, changed fields,
source path, and edit warnings. The operation does not rename, move, delete,
change flavour or relations, commit to Git, or maintain history.
:::

:::mara design DES-ITEM-UPDATE
:mid: 01M1RSQNHERJ070G06PK18A77K
:title: Apply validated partial updates to source spans
:satisfies: REQ-ITEM-UPDATE
:satisfies: REQ-SURFACE-PARITY

## Input

CLI: `item update <reference> [--title <title>] [--field <key=value>]`
`[--clear-field <key>] [--body <text|->]`. Both field options are repeatable;
`--body -` reads standard input. MCP accepts `reference`, optional `title`,
`fields: [{key, value}]`, `clear_fields: [key]`, and `body`, plus the usual
project selection. MCP body text is literal, including `-`. Omitted properties
request no change; null `title` or `body` is equivalent to omission.

Group supplied field values by key in request order. Each group replaces that
field's complete value sequence. Clearing removes all entries of an optional
custom field; clearing an absent optional field succeeds without a change.
Reject unknown or structural field names, relation names, and any key both set
and cleared. Trim title and field values using the existing scalar rules and
reject embedded line breaks. An empty string value differs from clearing a
field. Requiredness requires presence, and field types follow the schema.

## Source editing and validation

Use parser metadata and body byte spans rather than rendering a new item block.
Reuse existing field occurrences in order, retaining their surrounding value
whitespace. Remove surplus occurrences and place extra values immediately after
the last occurrence. Append newly introduced fields after existing metadata in
key order. Leave semantically unchanged fields byte-identical. Preserve the
blank metadata/body boundary and closing delimiter, including its terminator.
Normalize replacement body LF/CRLF line endings and new metadata lines to the
document's first newline style (LF by default). A nonempty replacement body
receives a final newline if needed; an empty body becomes zero bytes.

Require unambiguous existing identities. Reparse the candidate and validate the
whole candidate corpus. Permit only typed missing-body diagnostics belonging to
unchanged existing scaffold bodies when that body was not explicitly replaced.
Every other validation diagnostic blocks writing. Verify unchanged item count,
identities, and all metadata outside the request, exact replacement body
recognition, and unchanged bodies and mentions on other items. Reject body
content that escapes the item or hides another item in Markdown context.

Stage the candidate in the original file's parent with preserved permissions.
Before atomic replacement, recheck the original source and permissions, project
configuration, schema, discovery, and corpus. Use the existing mutation lock and
single-file staging without a multi-file journal. Concurrent manual filesystem
edits during publication remain outside that advisory lock.

## Result

Return `{id, mid, path, changed_fields, warnings}`. `path` is project-relative;
`changed_fields` contains actual changed custom keys and/or `title` and `body`,
unique and sorted lexically. A request with no effective change succeeds with an
empty list and no file replacement. `warnings` uses the validation diagnostic
shape `{scope, path, line, message}` for each remaining scaffold body, including
scaffolds in other documents; locations refer to the resulting corpus. Human
output labels each as `warning` on stderr. CLI JSON and MCP return the same
warning array, empty when the resulting corpus has no diagnostics.
:::

:::mara decision ADR-DRAFT-ITEM-UPDATES
:mid: 01M1RSQNHVC39H4Y9CCSTW1GQV
:title: Allow continued drafting with explicit edit warnings
:justifies: REQ-ITEM-UPDATE

Permit structured title and field updates while an existing scaffold's required
body remains missing. Requiring a fully valid project after every edit would
block incremental drafting supported by item creation. Return the remaining
missing-body diagnostics as warnings during update, while explicit validation
continues to fail until the bodies are supplied. This exception does not allow
new missing required bodies or other invalid state.
:::
