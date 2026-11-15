# Write safety

## Requirements

---

<a id="01M7BF6HHH01T5XJNAF7DM433R"></a>

### A write touches only what it must

| Type | Requirement |
| --- | --- |
| ID | REQ-MINIMAL-REWRITE |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

A write changes only the spans its operation requires. All other bytes, including the author's Markdown formatting and line endings, and the file's permissions are preserved.

---

<a id="01M7BF6HHTMM6D6QAF0BHFBCE6"></a>

### A refused edit changes nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-REFUSED-EDIT-CHANGES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit that Mara refuses leaves every file byte-identical. Mara refuses an edit when it finds, before its first write, that a file it would write changed after the snapshot the edit was computed against.

---

<a id="01M7BF6HJ3758Y06V61901ZSKK"></a>

### An interrupted edit loses nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-INTERRUPTED-EDIT-LOSES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit interrupted between files, or stopped because a file changed after its first write, never loses an item. It leaves at worst a duplicate item, stale link paths, stale link text, code markers whose ID does not resolve yet, or a 0.3.0 project that rerunning the conversion completes, each reported as a diagnostic.

---

<a id="01M7CXC2N8EZYVHGY7T2GPPB3X"></a>

### Mara's writers never overwrite each other

| Type | Requirement |
| --- | --- |
| ID | REQ-WRITERS-SERIALIZED |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

When two Mara processes edit the same project at the same time, neither edit is lost: the second one is computed after the first, or is refused.

---

## Design

---

<a id="01M7BF6HJA1E3SE8WJH77FGP17"></a>

### Write procedure

| Type | Design |
| --- | --- |
| ID | DES-WRITE-PROCEDURE |
| Status | Accepted |
| Satisfies | [REQ-REFUSED-EDIT-CHANGES-NOTHING](#01M7BF6HHTMM6D6QAF0BHFBCE6) |
| Satisfies | [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](#01M7BF6HJ3758Y06V61901ZSKK) |
| Satisfies | [REQ-WRITERS-SERIALIZED](#01M7CXC2N8EZYVHGY7T2GPPB3X) |

For each edit:

1. Take the project's write lock: an exclusive advisory lock on the `.mara` directory, held until the edit ends. Every Mara writer takes it: a direct CLI call, a daemon of any version, an embedded host.
2. Compute the edit, then check that every file it will write still has the hash recorded in the snapshot it was computed against. A mismatch refuses the edit.
3. For each file, in the order below: write a temporary file, check the target's hash once more, and rename the temporary file into place.
4. A host that keeps state between requests re-derives only the changed files. A direct CLI call does no read, hash, parse or snapshot build after the renames ([ADR-NO-REDERIVATION-IN-CLI](#01M7FAG31QX231C198D72N3FN9)).

- A mismatch in step 3 after the first rename stops the edit and leaves an interrupted edit ([REQ-INTERRUPTED-EDIT-LOSES-NOTHING](#01M7BF6HJ3758Y06V61901ZSKK)).
- The lock binds only Mara. A save by another program between the last check and the rename is not detected.
- Paths are checked as they are when the procedure uses them ([ADR-WRITE-PATHS-AS-USED](#01M7FGKVAQ87E074MAY6594DB3)). A directory of a target replaced while the edit runs, by a symbolic link for example, can redirect the temporary file's rename or removal and can leave the temporary file behind.
- The lock is `flock` on `.mara` opened as a directory; a writer waits while another holds it. A `.mara` that is a symbolic link is refused with `symbolic-link` at line 1 of the configuration, as [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) refuses it.
- `project init` creates `.mara` just before step 1 when it is absent ([DES-PROJECT-INIT](init.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)).
- A file that the edit creates must still be absent at both checks. Step 3 creates its missing directories just before its temporary file.
- A file the edit writes, or a directory between the project root and it, that is a symbolic link fails the check with `symbolic-link` at that file. The procedure never writes through or replaces a symbolic link.
- The temporary file lies in the target's directory under a short hidden name, whatever the target's length, that never ends in `.mara.md`, so it is no input. Once its contents are written, which clears set-user-ID and set-group-ID bits, it gets the target's permissions. It is flushed to disk before its rename, and its directory after the rename, so the order holds after a power loss. A failure removes it; when the removal fails, or a directory on its path became a symbolic link, the message names the temporary file instead of saying that nothing was written.
- A failed check fails the request with `changed-file` ([DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS)), a file that cannot be checked or written with `io-error`, each with a diagnostic at line 1 of that file. After the first rename the message lists the files already replaced.
- A debug build has pause points for tests, at two places ([ADR-DEBUG-PAUSE-POINT](#01M7FCGJEWJ4EG946TYEVJAMW1)). When the environment variable `MARA_TEST_PAUSE` names a directory, the writer stops once per edit after computing it and before step 2, and in step 3 of each file after writing that file's temporary file and before its second check. At each stop it creates `paused` in that directory, whole: empty at the stop before step 2, and otherwise holding the file's path relative to the project root. It goes on once `paused` is gone. A release build contains no trace of them: a case of the release-build `performance` test runs a write with the variable set, which completes without creating `paused`, and finds no `MARA_TEST_PAUSE` in the binary's bytes.

Order across files:

| Operation | Order | Rerunning an interrupted operation |
| --- | --- | --- |
| Move | Destination file, then removal from the source file, then inbound link paths | Works while the item is in both files: the move skips the first step when the destination already holds the item as the move would write it |
| Rename | Code markers that name the old ID and the text of mentions whose text is the old ID, then the item's `ID` row, then the text of inbound relation links ([ADR-MENTIONS-BEFORE-ID-ROW](#01M7FAG325QB1J18HQQQFMBJDS)) | Works while the old ID still resolves; after the `ID` row only stale relation link text is left |

Fix rewrites the stale links an interruption leaves ([REQ-ONE-FIX-OPERATION](authoring.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)).

---

## Decisions

---

<a id="01M7FAG31QX231C198D72N3FN9"></a>

### Re-derive after a write only in a host that keeps state

| Type | Decision |
| --- | --- |
| ID | ADR-NO-REDERIVATION-IN-CLI |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen by the maintainer on 2026-11-15: step 4 binds hosts that keep state, which re-derive only the changed files, never the whole project. A direct CLI call does no read, hash, parse or snapshot build after the renames: the edit was computed and checked before the writes.
- Rejected: re-deriving in a direct CLI call. The call keeps no state, so the work ends with the process.

---

<a id="01M7FGKVAQ87E074MAY6594DB3"></a>

### Check a write's paths as they are when it uses them

| Type | Decision |
| --- | --- |
| ID | ADR-WRITE-PATHS-AS-USED |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen by the maintainer on 2026-11-15: as for reads ([DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)), the write procedure checks each path as it is when it uses it. A program that can replace a project directory within the milliseconds of a write, and knows the hidden temporary file's name, can already delete that outside file itself. A directory moved during a write can leave one temporary file behind.
- Rejected: creating, removing and renaming the temporary file through directory handles. They need operating-system calls that the standard library lacks, are platform-specific, and add code on the most safety-critical path.
- Rejected: checking the path again before removing the temporary file. It narrows the window without closing it.

---

<a id="01M7FAG325QB1J18HQQQFMBJDS"></a>

### Rewrite mention text before the `ID` row

| Type | Decision |
| --- | --- |
| ID | ADR-MENTIONS-BEFORE-ID-ROW |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen by the maintainer on 2026-11-15: a rename rewrites code markers and mention text, then the `ID` row, then inbound relation link text. A rename stopped before the `ID` row is completed by a rerun; one stopped after it leaves only stale relation link text, which validation reports and fix rewrites.
- Rejected: mention text after the `ID` row, with the relation link text. A stop after the `ID` row would leave mentions with the old ID that no diagnostic reports, since mention text is free, and that no rerun reaches.

---

<a id="01M7FCGJEWJ4EG946TYEVJAMW1"></a>

### Test the write procedure's checks through pause points in debug builds

| Type | Decision |
| --- | --- |
| ID | ADR-DEBUG-PAUSE-POINT |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen by the maintainer on 2026-11-15: one pause point before each file's second check, compiled only into debug builds, which the tests run. A test makes the writer wait, changes the file and lets it continue. It is deterministic on a busy machine and a few lines of code, and it is test-only code in the writer.
- Rejected: no hook, the mechanism left to the writer issue. No event outside the process falls between the two checks, so the test would race, retry and may flake under load.
- Rejected: a unit test below the CLI. It departs from tests that run the real CLI and needs a seam in the write function.
- Chosen by the maintainer on 2026-11-15, later: the same mechanism also pauses once per edit, after the edit is computed and before step 2. A test changes a later file there and expects a refusal with nothing replaced. It stays in debug builds only, with no general fault-injection facility.
- Rejected: verifying step 2 by inspection. A regression that drops step 2 would fail no test.
- Rejected: accepting either a refusal or an interrupted edit for a later file changed before writing began. The refusal of [REQ-REFUSED-EDIT-CHANGES-NOTHING](#01M7BF6HHTMM6D6QAF0BHFBCE6) would then not be shown for multi-file edits.
