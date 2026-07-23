# Git editing

Mara v0.1 performs one repository-wide structured edit: display-ID renaming.
That operation establishes the safety contract later web and agent editors must
follow—precise patches, explicit preflight, recoverable writes, and no hidden
commit.

## Rename preflight

:::req m_01KY7YA2DCGN8Z56K6PFDFM2JQ
:id: REQ-EDIT-RENAME-PREFLIGHT
:title: Display-ID rename shall complete semantic preflight before writing
:status: approved
:level: system
:kind: functional
:priority: must
:refines: REQ-DISPLAY-ID-RENAME

Before modifying a file, Mara shall resolve exactly one source item, validate
the replacement ID against its flavour and project-wide uniqueness, locate each
resolved internal display-ID occurrence, and prove that every planned patch
matches the source bytes originally parsed.
:::

:::req m_01KY7YA2DDSNS0YZ1BZ1XZAH9H
:id: REQ-EDIT-WORKTREE-POLICY
:title: Repository-wide writes shall require a clean Git worktree by default
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

When the project is in Git and clean-write policy is enabled, Mara shall refuse
to rename while tracked or relevant untracked changes exist. `--allow-dirty`
shall explicitly override that policy without weakening source preimage checks
or transaction recovery.
:::

:::req m_01KY7YA2DE4S3C8FM9KHHCEW0H
:id: REQ-EDIT-MINIMAL-PATCHES
:title: Structured edits shall change only affected source spans
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

A rename shall replace the item's display-ID value and reference target tokens
that resolve to that display ID. It shall preserve MID references, labels,
unrelated matching prose, metadata order, whitespace, Markdown layout, file
permissions, encoding, and existing line-ending style.
:::

## Multi-file transaction

:::req m_01KY7YA2DFYREBEV8P5WHDSM65
:id: REQ-EDIT-STAGING
:title: Mara shall stage every replacement before changing originals
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-RENAME-DISPLAY-ID

Mara shall compute all new file bytes, validate the post-edit project model, and
write staged replacements on the same filesystems as their destinations before
replacing any original file.
:::

:::req m_01KY7YA2DGN3CW9Q6N6D5E0W37
:id: REQ-EDIT-JOURNAL
:title: Multi-file writes shall use a durable recovery journal
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: GOAL-AUDITABLE

Before replacement begins, Mara shall record a transaction journal under
`.mara/transactions/` containing operation identity, affected project-relative
paths, original content digests, staged content digests, backup locations, and
per-file progress. Journal updates shall be durable enough to detect interruption.
:::

:::req m_01KY7YA2DHXPRFZK1JPCV1B5DA
:id: REQ-EDIT-ROLLBACK
:title: Mara shall restore originals after a failed rename
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

If a write fails during the running process, Mara shall restore every replaced
file from the transaction backups and report the operation and recovery result.
It shall retain the journal whenever successful restoration cannot be proven.
:::

:::req m_01KY7YA2DJFPWN6AC4SQNGT7K8
:id: REQ-EDIT-RECOVERY
:title: Mara shall require explicit recovery after an interrupted transaction
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

When an incomplete journal exists, every mutating command shall stop and explain
the transaction state. `mara transaction recover --rollback` shall restore the
recorded originals, while `--complete` shall apply the remaining staged files
only when all recorded preconditions still hold.
:::

:::req m_01KY7YA2DKG5P04ZDCHEMN44ZG
:id: REQ-EDIT-NO-COMMIT
:title: Mara shall not commit source edits automatically
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-GIT-CANONICAL

Initialization, rename, recovery, and later structured-edit commands shall leave
the resulting working-tree changes for explicit human or external workflow
review. Mara shall not create a branch, commit, push, or pull request unless a
separate future integration command is invoked explicitly.
:::

:::req m_01KY7YA2DM0Y3WQ3FJ148JDP8Q
:id: REQ-EDIT-POSTCHECK
:title: Mara shall verify the complete project after a successful edit
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID

Before deleting transaction backups, Mara shall reparse and validate the final
working tree and confirm that the old display ID no longer resolves, the new ID
resolves to the original MID, and every intended rewritten relation preserves
its canonical endpoints.
:::

## Design, rationale, risk, and artifact

:::design m_01KY7YA2DSZC70VTD3AVAKCM8B
:id: DES-SOURCE-TRANSACTION
:title: Span-checked recoverable source transaction
:status: accepted
:kind: component
:satisfies: REQ-EDIT-RENAME-PREFLIGHT
:satisfies: REQ-EDIT-WORKTREE-POLICY
:satisfies: REQ-EDIT-MINIMAL-PATCHES
:satisfies: REQ-EDIT-STAGING
:satisfies: REQ-EDIT-JOURNAL
:satisfies: REQ-EDIT-ROLLBACK
:satisfies: REQ-EDIT-RECOVERY
:satisfies: REQ-EDIT-NO-COMMIT
:satisfies: REQ-EDIT-POSTCHECK

The editing service shall consume parsed source spans and immutable preimage
bytes, build a complete patch plan, validate staged postimages, and execute
per-file atomic replacement under a recoverable project transaction. The same
service is the future write boundary for web and agent editing.
:::

:::decision m_01KY7YA2DTENB5QEDPE5AS3W70
:id: ADR-0008
:title: Make source edits reviewable Git working-tree changes
:status: accepted
:kind: process
:justifies: DES-SOURCE-TRANSACTION
:justifies: REQ-EDIT-NO-COMMIT

Mara owns semantic patch correctness but not delivery policy. Leaving changes
uncommitted preserves existing Git hooks, review conventions, branch policies,
and external orchestration such as Linear or GitHub.
:::

:::risk m_01KY7YA2DVVWCAJ6XPGYN39AMS
:id: RISK-PARTIAL-RENAME
:title: Interrupted multi-file rename may leave mixed display IDs
:status: open
:severity: critical
:likelihood: low
:affects: REQ-EDIT-JOURNAL
:affects: REQ-EDIT-ROLLBACK
:affects: REQ-EDIT-RECOVERY
:affects: DES-SOURCE-TRANSACTION

Filesystem replacement cannot be globally atomic across multiple paths. A crash
or storage error can leave only part of a rename visible, so staged content,
backups, journaling, and mandatory recovery are part of correctness rather than
optional convenience.
:::

:::artifact m_01KY7YA2DW3XSRD0509K8FM70R
:id: ART-TRANSACTION-JOURNAL
:title: Mara source transaction journal
:status: proposed
:kind: file_format
:uri: .mara/transactions/

The ignored local transaction directory contains staged files, backups, and a
versioned journal needed to complete or roll back interrupted structured edits.
:::

## Planned verification

:::test m_01KY7YA2DNNDSGPFNDFC0Z4TEG
:id: TEST-EDIT-PREFLIGHT
:title: Rename preflight and minimal patch test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-EDIT-RENAME-PREFLIGHT
:verifies: REQ-EDIT-WORKTREE-POLICY
:verifies: REQ-EDIT-MINIMAL-PATCHES

Fixtures shall cover clean and dirty worktrees, explicit override, stale source
preimages, MID and display-ID references, labels, unrelated matching prose,
metadata ordering, Unicode, file permissions, LF, and CRLF.
:::

:::test m_01KY7YA2DP4SA2C8NSP17GEGZE
:id: TEST-EDIT-TRANSACTION
:title: Multi-file rename transaction failure test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-EDIT-STAGING
:verifies: REQ-EDIT-JOURNAL
:verifies: REQ-EDIT-ROLLBACK

Fault injection shall fail staging, journal persistence, and each replacement
position. Tests shall prove either unchanged originals or successful in-process
rollback with an accurate retained journal when restoration is uncertain.
:::

:::test m_01KY7YA2DQNNG0Z8EAM4CBHZ8V
:id: TEST-EDIT-RECOVERY
:title: Interrupted transaction recovery acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-EDIT-RECOVERY
:verifies: REQ-EDIT-NO-COMMIT
:verifies: REQ-EDIT-POSTCHECK

Process-interruption fixtures shall exercise rollback and completion from every
journal progress state, changed preconditions, final full-project validation,
absence of automatic commits, and cleanup only after proven success.
:::
