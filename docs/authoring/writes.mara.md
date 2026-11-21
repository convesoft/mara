# Write safety

## Requirements

---

<a id="01M7BF6HHH01T5XJNAF7DM433R"></a>

### A write touches only what it must

| Type | Requirement |
| --- | --- |
| ID | REQ-MINIMAL-REWRITE |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

A write changes only the spans its operation requires. All other bytes, including the author's Markdown formatting and line endings, and the file's permissions are preserved.

---

<a id="01M7BF6HHTMM6D6QAF0BHFBCE6"></a>

### A refused edit changes nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-REFUSED-EDIT-CHANGES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit that Mara refuses leaves every file byte-identical. Mara refuses an edit when it finds, before its first write, that a file it would write changed after the snapshot the edit was computed against.

---

<a id="01M7BF6HJ3758Y06V61901ZSKK"></a>

### An interrupted edit loses nothing

| Type | Requirement |
| --- | --- |
| ID | REQ-INTERRUPTED-EDIT-LOSES-NOTHING |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

An edit interrupted between files, or stopped because a file changed after its first write, never loses an item. It leaves at worst a duplicate item, stale link paths, stale link text, code markers whose ID does not resolve yet, or a 0.3.0 project that rerunning the conversion completes, each reported as a diagnostic.

---

<a id="01M7CXC2N8EZYVHGY7T2GPPB3X"></a>

### Mara's writers never overwrite each other

| Type | Requirement |
| --- | --- |
| ID | REQ-WRITERS-SERIALIZED |
| Status | Accepted |
| Satisfies | [NEED-SAFE-AUTHORING](../product.mara.md#01M7BF6H67YSV2DQ3QYBVQCN9V) |

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
- The lock is `flock` on `.mara` opened as a directory; a writer waits while another holds it. A `.mara` that is a symbolic link is refused with `symbolic-link` at line 1 of the configuration, as [DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) refuses it.
- `project init` creates `.mara` just before step 1 when it is absent ([DES-PROJECT-INIT](../project.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)).
- A file that the edit creates must still be absent at both checks. Step 3 creates its missing directories just before its temporary file.
- A file the edit writes, or a directory between the project root and it, that is a symbolic link fails the check with `symbolic-link` at that file. The procedure never writes through or replaces a symbolic link.
- The temporary file lies in the target's directory under a short hidden name, whatever the target's length, that never ends in `.mara.md` and that the inventory never takes for a code file, so it is no input. Once its contents are written, which clears set-user-ID and set-group-ID bits, it gets the target's permissions. It is flushed to disk before its rename, and its directory after the rename, so the order holds after a power loss. A failure removes it; when the removal fails, or a directory on its path became a symbolic link, the message names the temporary file instead of saying that nothing was written.
- A failed check fails the request with `changed-file` ([DES-OPERATIONS](../hosts/index.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS)), a file that cannot be checked or written with `io-error`, each with a diagnostic at line 1 of that file. After the first rename the message lists the files already replaced.
- A debug build has pause points for tests, at two places ([ADR-DEBUG-PAUSE-POINT](#01M7FCGJEWJ4EG946TYEVJAMW1)). When the environment variable `MARA_TEST_PAUSE` names a directory, the writer stops once per edit after computing it and before step 2, and in step 3 of each file after writing that file's temporary file and before its second check. At each stop it creates `paused` in that directory, whole: empty at the stop before step 2, and otherwise holding the file's path relative to the project root. It goes on once `paused` is gone. A release build contains no trace of them: a case of the release-build `performance` test runs a write with the variable set, which completes without creating `paused`, and finds no `MARA_TEST_PAUSE` in the binary's bytes.

Order across files:

| Operation | Order | Rerunning an interrupted operation |
| --- | --- | --- |
| Move | Destination file, then removal from the source file, then inbound link paths | Works while the item is in both files: the move skips the first step when the destination already holds the item as the move would write it, wherever in that file |
| Rename | Code markers that name the old ID and the text of mentions whose text is the old ID, then the item's `ID` row, then the text of inbound relation links ([ADR-MENTIONS-BEFORE-ID-ROW](#01M7FAG325QB1J18HQQQFMBJDS)) | Works while the old ID still resolves; after the `ID` row only stale relation link text is left |
| Fix | Each changed document in path order ([DES-FIX-REPAIRS](index.mara.md#01M7FWM48HHP0HHM6S86P6R499)) | Works: each document's repairs depend on no other document |
| Conversion | The report file, each changed document in path order, the schema, the configuration last ([DES-CONVERSION-MAPPING](../migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM)) | Works: until the configuration is written the project is a 0.3.0 project, which a rerun completes ([ADR-CONVERSION-RERUN](../migration.mara.md#01M7F9N0J8E9GJPCEATS4MQX7T)) |

- A file is written once, with every change the edit makes to it, at the first step that changes it, other files of one step in path order. A rename writes the item's file at the `ID` row step, after every other file with mention text.

Fix rewrites the stale links an interruption leaves ([REQ-ONE-FIX-OPERATION](index.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH)).

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

- Chosen by the maintainer on 2026-11-15: as for reads ([DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)), the write procedure checks each path as it is when it uses it. A program that can replace a project directory within the milliseconds of a write, and knows the hidden temporary file's name, can already delete that outside file itself. A directory moved during a write can leave one temporary file behind.
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
- Chosen during the rewrite on 2026-11-21 (mara-zmp8yz): one more pause point, outside the writer, in debug builds only. When `MARA_TEST_PAUSE_EVALUATION` names a directory, a rule evaluation that began stops before it checks whether the daemon stops, creates `paused` there and goes on once it is gone, so a test stops the daemon at that point ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)). Its name begins with `MARA_TEST_PAUSE`, so the release-build check of the pause points finds it too.
- Rejected: stopping the daemon at a random moment during an evaluation. The window between counting an evaluation and creating its directory is a few instructions wide, so such a test would almost never reach it.

---

<a id="01M7BF6GW5PH28XAS4VF6EJF8G"></a>

### Order multi-file writes instead of journaling them

| Type | Decision |
| --- | --- |
| ID | ADR-NO-MUTATION-JOURNAL |
| Status | Accepted |
| Justifies | [DES-WRITE-PROCEDURE](#01M7BF6HJA1E3SE8WJH77FGP17) |

- Chosen: check every target before the first write, then order the writes so that an interruption leaves only states validation reports. The MID in each link fragment keeps identity intact meanwhile, and one write lock per project serializes Mara's writers.
- Rejected: a journal of preimages with an explicit rollback command, as in 0.3.0. It adds transaction state that blocks writers, and recovery code.
- Rejected: fix removing one copy after an interrupted move. Fix would delete an item and guess between the copies.
