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
:title: Recover interrupted multi-file mutations without losing source data

Before replacing any original, movement and rename verify every planned edit
against its preimage, stage every candidate, and validate the complete candidate
corpus. Their journaled replacements record durable project-local recovery
information first.
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

:::mara requirement REQ-ITEM-DELETION
:mid: 01M1RTTGFNW3Y7K8CQW16FJKYV
:title: Delete an item only when surviving references remain valid

`item delete <reference>` and MCP `item_delete` remove exactly one item
resolved by exact MID or human ID. Require a valid complete project before
editing and a valid complete surviving corpus before publication.

Refuse deletion when any surviving item's typed relation or supported wiki
mention resolves to the selected MID or human ID. Report every blocking
occurrence with its source path, one-based line, and byte span. References
inside the selected item, including self-references, disappear with it and do
not block deletion. Supported mentions follow [[DES-DOCUMENT-FORMAT]].

Remove only the selected block and minimally normalize adjacent empty lines.
Preserve unrelated source bytes, surviving items and references, line-ending
style, and file permissions. Keep the containing document even when empty.
Validate before one atomic replacement under the existing mutation lock;
pending recovery state blocks deletion under [[REQ-RECOVERABLE-MUTATION]].

Human output, CLI JSON, and MCP identify the deleted MID, human ID, and source
path. There is no cascade, force option, tombstone, alias, history record,
document deletion, bulk operation, or Git operation.
:::

:::mara design DES-ITEM-DELETION
:mid: 01M1RTTGG2S7CSTJTG2ES0K9RM
:title: Preflight references before removing an item source span
:satisfies: REQ-ITEM-DELETION
:satisfies: REQ-SURFACE-PARITY

CLI accepts `item delete <reference>`. MCP accepts `reference` and the usual
optional project selection. Both use one shared operation and return
`{id, mid, path}`, with the original project-relative document path.

Load and validate the full corpus under the project mutation lock, then resolve
one exact identity. Scan every surviving item's schema-defined typed relations
and parser-recognized body mentions, resolving both human IDs and MIDs. Retain
every blocking occurrence, ordered by document path and source byte offset.
Invocation errors follow [[DES-COMMAND-SURFACE]]; a blocked deletion names the
selected identities and lists each source item, relation name or mention,
path, one-based line, and end-exclusive UTF-8 byte span. Authors must remove or
redirect those references explicitly before retrying.

Remove the parser's complete item span, including the closing line terminator
when present. If removal joins an empty line before the block with an empty
line after it, remove exactly the first following empty line. An empty line
here contains only LF or CRLF. Preserve every other byte, including existing
leading/trailing blank lines, whitespace-only lines, and the final-newline
state of surviving content. Do not insert separators or delete the document.

Reparse the candidate, validate the complete surviving corpus, and verify that
exactly the selected item disappeared while every surviving block and its
recognized mentions remain unchanged. Stage the candidate in the source parent
with original permissions. Use the existing single-file transaction path to
recheck source bytes, permissions, project configuration, schema, discovery,
and the full corpus before atomic replacement, without a multi-file journal.
Concurrent manual filesystem edits during publication remain outside the
advisory lock, as described in [[DES-ITEM-MOVEMENT]].
:::

:::mara requirement REQ-ITEM-RENAME
:mid: 01M1RW50QWRZSKHK5TP4B4QMFV
:title: Rename human IDs without changing resolved identity

`item rename <reference> <new-id>` and MCP `item_rename` rename exactly one
item resolved by exact MID or human ID. Require a valid complete project,
a replacement matching the ID grammar and selected flavour prefix, and
project-wide uniqueness. Preserve the MID and flavour.

Rewrite only the opener ID and schema-defined typed-relation or supported body
wiki-mention targets authored with the old human ID, including self-references.
Supported mentions follow [[DES-DOCUMENT-FORMAT]]. Preserve MID-authored targets,
labels, other metadata values, unrelated prose, code and raw contexts, metadata
order, whitespace, Markdown layout, existing line endings, and file permissions.

Validate the complete candidate corpus before publication; every relation and
mention must retain its MID endpoints. Use [[REQ-RECOVERABLE-MUTATION]] for
publication and recovery. On success, the new ID resolves to the original MID
and the old ID no longer resolves. Requesting the current ID succeeds without
writing and returns no affected paths.

Human output, CLI JSON, and MCP identify the unchanged MID, old and new IDs,
and affected paths. No alias, MID rename, flavour change, external or historical
rewrite, bulk rename, history record, or Git operation is included.
:::

:::mara design DES-ITEM-RENAME
:mid: 01M1RW50RC4BK3RAH78AEJVR2T
:title: Patch parsed ID targets through the recoverable transaction
:satisfies: REQ-ITEM-RENAME
:satisfies: REQ-RECOVERABLE-MUTATION
:satisfies: REQ-SURFACE-PARITY

CLI accepts `item rename <reference> <new-id>`. MCP accepts `reference`,
`new_id`, and the usual optional project selection. Both invoke one operation
and return `{mid, old_id, new_id, paths}`. Paths are unique project-relative
changed document paths in lexical order, empty for an unchanged ID.

Hold the project mutation lock and validate the full corpus before resolution
and replacement-ID checks. Plan byte patches from the selected item's opener,
schema-defined relation metadata spans, and parser-recognized mention spans.
Check each opener, metadata scalar, and mention against its parsed preimage;
replace only the human-ID token. Reject overlapping or mismatched patches and
apply them in reverse byte order per file without rendering Markdown.
Labelled wiki syntax is not introduced; unsupported syntax remains literal.

Reparse all changed documents and validate the complete projected corpus.
Verify the same item MIDs, expected human IDs, flavours, and document paths,
and the same ordered relation names and resolved relation and mention MID
endpoints. Stage every changed file with original permissions and recheck the
project configuration, schema, discovery, corpus, and file preimages before
replacing any original.

Publish every nonempty rename through the journal format 1 transaction in
[[DES-ITEM-MOVEMENT]], including a rename affecting only one document. Its
rollback, interrupted-process recovery, pending-state blocking, manual-edit
protection, and platform durability limits apply unchanged. No-op renames still
require a valid corpus and an available mutation lock.
:::

:::mara decision ADR-RENAME-WITHOUT-ALIASES
:mid: 01M1RW50RVQY5MM2V2S7FQFCD1
:title: Keep one current human handle per durable identity
:justifies: REQ-ITEM-RENAME

Rename replaces the current human-readable handle without retaining aliases.
The immutable MID already provides a stable target when a reference must survive
human-ID changes. Keeping alias state would add a second persisted naming
contract and ambiguous future ID reuse. Rewrite supported current-corpus human
references in the same recoverable transaction; external systems and historical
revisions retain their authored text and are outside the rename boundary.
:::
