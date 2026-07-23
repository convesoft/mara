# Mara source transaction protocol v1

This reference groups the typed design contracts for v0.1 multi-file source
transactions. Each contract below is independently addressable in the Mara graph.

:::design m_01KY82WX4Q2767NS30BXX5R16Q
:id: DES-TRANSACTION-JOURNAL-STORAGE
:title: Transaction layout, journal data, and durable journal updates
:status: accepted
:kind: storage
:satisfies: REQ-EDIT-RENAME-PREFLIGHT
:satisfies: REQ-EDIT-STAGING
:satisfies: REQ-EDIT-JOURNAL
:depends_on: DES-SOURCE-TRANSACTION

A conforming implementation shall refuse to begin a mutation when the filesystem
cannot provide durable file flush, atomic same-filesystem replacement, and a
durable parent-directory or platform-equivalent metadata flush.

## Layout and identifiers

Each transaction has an ID `tx_` followed by a canonical uppercase ULID. Its
control directory is `.mara/transactions/<transaction-id>/` and contains
`journal.json`. For every affected destination, Mara exclusively creates a stage
file and backup file in the destination's parent directory so replacement stays
on the destination filesystem. Their names are
`.mara-<transaction-id>-<ordinal>.stage` and `.backup`; ordinal is a zero-padded
six-digit index in normalized path order.

All paths recorded by the journal are normalized project-relative paths. v0.1
edit preflight rejects affected symbolic links, non-regular files, duplicate
filesystem identities, and any destination or temporary path outside the project
root. Temporary files use exclusive no-follow creation and owner-only permissions
until final destination permissions are applied.

## Journal JSON

The journal uses the canonical JSON encoding from the wire-contract reference.
Its keys and value shapes are:

```json
{
  "format": "mara.transaction",
  "version": 1,
  "id": "tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R",
  "operation": "display_id_rename",
  "phase": "preparing",
  "outcome": null,
  "allow_dirty": false,
  "source_mid": "m_01JX0TV1P2V1N0VJ3M3J6W9Y7R",
  "old_id": "REQ-OLD-001",
  "new_id": "REQ-NEW-001",
  "files": [
    {
      "ordinal": 0,
      "path": "docs/capabilities/example.mara.md",
      "file_identity": "device:2049;inode:123456",
      "original_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "replacement_sha256": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      "original_size": 1234,
      "replacement_size": 1234,
      "readonly": false,
      "unix_mode": 420,
      "stage_path": "docs/capabilities/.mara-tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R-000000.stage",
      "stage_identity": null,
      "backup_path": "docs/capabilities/.mara-tx_01JX0TV1P2V1N0VJ3M3J6W9Y7R-000000.backup",
      "backup_identity": null,
      "state": "declared"
    }
  ]
}
```

This example is the initial durable journal written before any stage or backup
file is created. A display-ID rename always has at least one affected file because
the declaration of the renamed display ID is itself patched.

The operation set contains only `display_id_rename` in v1. `outcome` is null
until forward verification or rollback establishes `replacement` or `original`.
`source_mid`, `old_id`, and `new_id` are non-null strings. A file entry has keys:

`ordinal`, `path`, `file_identity`, `original_sha256`, `replacement_sha256`,
`original_size`, `replacement_size`, `readonly`, `unix_mode`, `stage_path`,
`stage_identity`, `backup_path`, `backup_identity`, and `state`.

`ordinal` starts at zero. `file_identity` is the platform's stable volume/device
and file-index/inode tuple encoded as an opaque non-empty string; an implementation
that cannot obtain and recheck it shall refuse the mutation. SHA-256 values use
lowercase hex. Sizes are byte counts. `unix_mode` is the original permission bits
or null on non-Unix platforms; `readonly` is always present. `state` is
`declared`, `staged`, `pending`, `applied`, `restored`, or `cleaned`. Stage and
backup identities begin null. A state transition and newly learned identity are
made durable after each individual preparation, replacement, restoration, or
cleanup step.

The journal contains no absolute path. Unknown format, version, operation, phase,
key, missing file, duplicate path/identity, invalid digest, or path-containment
failure stops automatic recovery and reports a conflicted transaction.

## Durable journal update

Every journal transition uses this sequence:

1. Serialize the complete next journal to `journal.next` in the transaction
   directory using exclusive creation or truncation of the prior next file.
2. Flush `journal.next` file data and metadata to stable storage.
3. Atomically replace `journal.json` with `journal.next` in the same directory.
4. Flush the transaction directory metadata.

Initial creation also flushes `.mara/transactions/`. No filesystem mutation that
depends on a new phase may begin until the corresponding journal update is
durable. On Windows, write-through replacement and directory-handle flushing or
an equivalent documented durable primitive is required; a best-effort rename is
not sufficient for v1 transactional writes.

### Journal-file crash reconciliation

Startup resolves the journal files before interpreting any transaction phase:

- when only `journal.json` exists, it is the committed journal;
- when both files exist, `journal.json` remains authoritative. Mara validates
  `journal.next` as the same transaction and either the same state or exactly one
  legal next state with unchanged immutable fields. It then removes
  `journal.next`, flushes the transaction directory, and reconciles the filesystem
  from `journal.json`; physical work performed before the interrupted update is
  detected by the per-file rules below;
- when only `journal.next` exists, it may be adopted only as the initial journal:
  phase is `preparing`, outcome is null, every file is `declared`, stage and backup
  identities are null, every destination still has its recorded original identity
  and digest, and no stage or backup exists. Mara atomically renames it to
  `journal.json`, flushes the transaction directory, and then continues normally;
- an exactly empty, validly named transaction directory contains no durable
  transaction and may be removed followed by a flush of `.mara/transactions/`;
- every other next-only state, malformed or cross-transaction next file, unknown
  entry, link, or non-regular file is a conflict and receives no automatic write.

The initial journal update occurs before creation of any stage or backup and
before every source mutation. Therefore the next-only adoption rule cannot hide
an unrecorded source change. Removing a recognized interrupted `journal.next`
never advances the state machine; only the committed journal plus idempotent
filesystem reconciliation can do so.

:::

:::design m_01KY82WX4RBJDT8Z7GY0W3JZVP
:id: DES-TRANSACTION-FORWARD-PROTOCOL
:title: Forward source transaction state machine
:status: accepted
:kind: algorithm
:satisfies: REQ-EDIT-MINIMAL-PATCHES
:satisfies: REQ-EDIT-POSTCHECK
:depends_on: DES-TRANSACTION-JOURNAL-STORAGE

## Forward state machine

The complete forward phase sequence is:

```text
preparing -> prepared -> applying -> applied -> verifying -> verified
          -> cleaning -> complete
```

The protocol is:

1. **Preflight, no journal:** parse and resolve the complete project, validate the
   rename, capture every destination identity and original digest, compute every
   replacement byte sequence, and validate the complete proposed project model.
2. **preparing:** durably create the initial journal with every file `declared`
   before any stage or backup. For each file, recheck destination identity and
   digest, exclusively write and flush the replacement stage, flush its parent,
   record `stage_identity` and `state: staged`, and durably update the journal.
   Then copy the original bytes and permissions to the backup, flush backup and
   parent, record `backup_identity` and `state: pending`, and durably update the
   journal. Only then begin preparation of the next file.
3. **prepared:** after every stage and backup matches its recorded digest and all
   file states are `pending`, durably transition to `prepared`.
4. **applying:** durably transition before replacing the first destination. In
   ascending ordinal order, recheck destination identity and original digest,
   atomically replace it with its stage, apply required original permissions,
   flush the destination file and parent directory, change its state to `applied`,
   and durably rewrite the journal before proceeding to the next file.
5. **applied:** after every destination has replacement digest and every file state
   is `applied`, durably transition to `applied`.
6. **verifying:** durably transition, then rediscover, parse, resolve, and validate
   the on-disk project. Confirm the old display ID no longer resolves, the new ID
   resolves to `source_mid`, and canonical relation endpoints are unchanged.
7. **verified:** after successful postcheck, set `outcome: replacement` and
   durably transition to `verified`.
8. **cleaning:** durably transition before deleting any backup. In ascending
   ordinal order, verify the destination digest matches `outcome`, remove any
   remaining stage and backup, flush the parent, mark the file `cleaned`, and
   durably update the journal before cleaning the next file. Transition to
   `complete` only when every file is `cleaned`.
9. **complete:** remove the transaction directory and flush
   `.mara/transactions/`. A leftover complete directory is safe to remove during
   discovery after validating its journal.

Failure before `verified` initiates rollback. Failure during `cleaning` retains
the journal and can only complete cleanup; the verified replacement is already
the authoritative result.

:::

:::design m_01KY82WX4SKVQGTV57FH5RKBSM
:id: DES-TRANSACTION-RECOVERY-PROTOCOL
:title: Rollback and interrupted transaction recovery state machines
:status: accepted
:kind: algorithm
:satisfies: REQ-EDIT-ROLLBACK
:satisfies: REQ-EDIT-RECOVERY
:depends_on: DES-TRANSACTION-JOURNAL-STORAGE

## Rollback state machine

Rollback transitions from `preparing`, `prepared`, `applying`, `applied`,
`verifying`, or `verified` to:

```text
rolling_back -> rolled_back -> cleaning -> complete
```

Mara durably records `rolling_back` before restoring a file. Files are processed
in descending ordinal order. For each file:

- when destination digest is the replacement digest, the backup identity and
  original digest are verified, the backup atomically replaces the destination,
  original permissions are applied, and destination plus parent are flushed;
- when destination already has the original digest and expected identity, no
  replacement is needed, but Mara reapplies the recorded permissions and flushes
  the destination and parent before recording restoration;
- every other state is a conflict and stops automatic writes.

After confirming the original digest, the file state becomes `restored` and the
journal update is made durable before moving to the prior file. `rolled_back` is
recorded only when every original is proven restored; the same update sets
`outcome: original`. `cleaning` then processes one file at a time using the
forward cleanup protocol, including durable `cleaned` states. A failed rollback
retains all remaining recovery material.

## Crash reconciliation

On startup, any non-complete transaction directory blocks every mutating command.
After resolving `journal.json` and `journal.next` as specified above, Mara validates
the committed journal and reconciles each crash window according to phase and
file state:

- in `preparing`, a `declared` file may have neither temporary file or an
  exclusively named stage/backup with the recorded digest created immediately
  before the crash; rollback may remove either, while completion may adopt both
  only after recording their identities and durable `staged` then `pending`
  transitions;
- in `preparing`, a `staged` file requires its recorded valid stage; a missing
  backup is expected and rollback may remove the stage. A matching unrecorded
  backup may be adopted for completion after its identity is recorded;
- a `pending` file with original destination digest and valid recorded
  stage/backup remains pending;
- a `pending` file with replacement destination digest, destination identity equal
  to the recorded stage identity, absent stage, and valid recorded backup is a
  replacement whose finalization may have been interrupted. Mara opens the
  destination without following links, rechecks identity and digest through that
  handle, reapplies `readonly` and `unix_mode`, flushes destination data and
  metadata, flushes the parent directory, and only then promotes it to `applied`
  in a durable journal update;
- an `applied` file with replacement digest and recorded stage identity at the
  destination plus a valid recorded backup remains applied;
- during `rolling_back`, an `applied` file already holding its original digest
  must have destination identity equal to the recorded backup identity. Mara
  opens it without following links, rechecks identity and digest, reapplies the
  recorded permissions, flushes the destination and parent, and only then
  promotes it to `restored` in a durable journal update;
- during `rolling_back`, a `pending`, `staged`, or `declared` file that still has
  the original digest must have the original `file_identity`. Mara reopens it
  without following links, reapplies recorded permissions, flushes destination
  and parent, and only then records `restored`;
- a `restored` file must hold its original digest and either `file_identity` when
  it was never replaced or `backup_identity` when restoration replaced it;
- in `cleaning`, `outcome` determines the required destination digest. A stage or
  backup may independently be present or absent because deletion may have
  preceded its journal update. Each present temporary file must have the recorded
  identity and digest before removal; when both are absent the file is promoted
  to `cleaned` by a durable update;
- outside `preparing` and `cleaning`, an unexpected missing required backup/stage,
  digest, identity, or path is conflicted and receives no automatic write.

Digest and identity recognition alone never proves completion of a replacement
or restoration. The idempotent permission-application and durability sequence
above is mandatory after a crash because the interruption may have occurred
immediately after atomic replacement, permission application, destination flush,
or parent-directory flush. A durable per-file `applied` or `restored` journal
state is the only proof that the complete sequence finished.

`transaction recover --complete` is permitted for `prepared`, `applying`,
`applied`, `verifying`, `verified`, and `cleaning`. It is permitted for
`preparing` only when every stage and backup is present and valid or can be
adopted under the preparation rules, in which case Mara records each missing
per-file transition and then `prepared`; otherwise only rollback cleanup is
possible. Rollback from `preparing` requires every destination to retain its
original digest, removes only recognized transaction-named temporaries, marks
each file `cleaned`, and completes without requiring never-created files.
Completion is forbidden once `rolling_back` or `rolled_back` is recorded.

`transaction recover --rollback` is permitted through `verified` while every
required backup remains valid. It is forbidden in `cleaning` or `complete`, where
backups may already be gone. Both modes first perform reconciliation and stop on
conflict. There is no force flag that overwrites unrecognized user content.

:::

:::design m_01KY82WX4TAQJWA07PWXZGFKPD
:id: DES-TRANSACTION-SECURITY
:title: Transaction path, link, and recovery-material security
:status: accepted
:kind: storage
:satisfies: REQ-EDIT-WORKTREE-POLICY
:depends_on: DES-TRANSACTION-JOURNAL-STORAGE

## Security and ownership

Journal and temporary files are local derived recovery data and are ignored by
Git. Mara shall reject links at any recorded control or temporary path, verify
file identity after opening handles, and never follow a journal path outside the
project. Cleanup removes only paths exactly recorded by a valid journal with the
expected transaction-specific name.
:::
