# Authoring verification

---

<a id="01M7F74HWB3KEQCABC1WSAJ34H"></a>

### Create items at each position

| Type | Verification |
| --- | --- |
| ID | VER-CREATE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CREATE-ITEM](index.mara.md#01M7BF6GKV9FQZBDWDZQJVE164) |

Rust integration tests run `item create` with the real CLI on a disposable project. A request that supplies a title, status, required fields and body, and an ID that no item has or none, creates an item that has no diagnostic and that `get` of its ID returns.

| Case | Expectation |
| --- | --- |
| No position | The item ends the document |
| Line 1, a line in narrative, and a separator line between two items | The item starts at that line, after the separator that the writer puts there when the line before needs one ([DES-ITEM-WRITER](index.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)); the content around it is unchanged and no neighbour merges with it |
| A line inside an item | Refused |
| A new path that the include patterns match | The document is created and does not start with `---` |
| A new path outside the include patterns, ignored by a `.gitignore`, or below another project's root | Refused; no file or directory is created |
| An ID given | The item has that ID |
| No ID, title "Phishing-resistant auth", type prefix `REQ-` | The item has the ID `REQ-PHISHING-RESISTANT-AUTH` |
| Two creates | Each anchor holds a new canonical ULID that no other item has |
| A MID supplied as an option | Rejected with `invalid-argument` and exit status 2 |

---

<a id="01M7F74HX5JT4JVYSSWJ2990YZ"></a>

### Update one part of an item at a time

| Type | Verification |
| --- | --- |
| ID | VER-UPDATE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-UPDATE-ITEM](index.mara.md#01M7BF6GM3N7AHGMTQ9JMT69ZP) |

Rust integration tests run `item update` with the real CLI on a disposable project. The updated item has relations, a single-valued field, a repeatable field and a body; another item shares its file. `Status` counts as a field value.

| Case | Expectation |
| --- | --- |
| A new title | Only the heading text changes |
| A new `Status`, a new value of the single-valued field, and new values of the repeatable field | Only those rows change, one row per value |
| A new body | Only the body changes |
| `--clear-field` of the repeatable field | Only its rows go |
| Title, a field and the body in one request | Those three change |
| After each of these | The anchor, the `Type` and `ID` rows, the relation rows and the other item are byte-identical |
| A request that names a new type, ID or MID, or a relation | Rejected or refused; files byte-identical |

---

<a id="01M7F74HXVC6WYBTNRTHB7AP1W"></a>

### Delete only an item nothing else targets

| Type | Verification |
| --- | --- |
| ID | VER-DELETE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-DELETE-ITEM](index.mara.md#01M7BF6GMX0QVD78B1ZTMY7PKF) |

Rust integration tests run `item delete` with the real CLI on a disposable project.

| Case | Expectation |
| --- | --- |
| An item with outgoing relations and a mention of itself, which nothing else targets | Removed; `get` of its ID and of its MID fails with `unknown-item`; no other item or narrative changes and the project gains no diagnostic |
| An item targeted by a relation in another item (one through a stale path), a mention in another item's body and a mention in narrative | Refused, listing each of these locations with path and line; files byte-identical |
| An item whose ID a code marker names, from 0.4.0-alpha.5 (mara-yhxq5x); the same item after that marker is removed | Refused, listing the marker's path and line; files byte-identical. Removed |

---

<a id="01M7F74HYAKH2G83A806Y7A4HC"></a>

### Add and remove relation rows

| Type | Verification |
| --- | --- |
| ID | VER-EDIT-RELATIONS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-EDIT-RELATIONS](index.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |

Rust integration tests run `relation add` and `relation remove` with the real CLI on a disposable project whose schema allows some relations between its types and not others.

| Case | Expectation |
| --- | --- |
| `relation add` of an allowed relation to an item in the same file, and to one in another directory | One new row on the source item: the relation's label, then `[<ID>](#<MID>)` or `[<ID>](<relative path>#<MID>)`; `relation list` of the target shows it under the inverse name; the project gains no diagnostic |
| `relation add` of a relation the schema does not allow from the source's type to the target's | Refused; files byte-identical |
| `relation add` and `relation remove` given an inverse name | Refused, naming the forward request; files byte-identical |
| `relation remove` of an existing relation | That row is deleted and nothing else changes |

---

<a id="01M7F74HYRP3X859VN0623J9AV"></a>

### Move an item with its links

| Type | Verification |
| --- | --- |
| ID | VER-MOVE-ITEM |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-MOVE-ITEM](index.mara.md#01M7BF6GMB197M9NE49NZN0A29) |

Rust integration tests run `item move` with the real CLI on a disposable project. The moved item has a relation and a mention to an item in its own file and to one in another directory. Items in its file, in the destination file and in a third file target it with relations and mentions.

| Case | Expectation |
| --- | --- |
| Move to a document in another directory | The anchor, heading, rows and body are unchanged, except that the paths of the item's own links resolve from the new file; every inbound relation link and mention carries the new path, `#<MID>` within the destination; the project gains no diagnostic |
| Move to another position in the same document | The item is at that position, byte-identical; the document's other regions keep their contents |
| `get` and `relation list` of the MID after each move | The item at its new location, with every relation and mention |

---

<a id="01M7F74HZCX3RZJVCNDJEKVW18"></a>

### Rename an ID wherever it is a handle

| Type | Verification |
| --- | --- |
| ID | VER-RENAME-ID |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-RENAME-ID](index.mara.md#01M7BF6GMNEZ9PYCWYKQZGY9YN) |

Rust integration tests run `item rename` with the real CLI on a disposable project. Other items target the renamed item with relations, with mentions whose text is its ID and with mentions whose text is other words. Narrative mentions it with its ID as the text, and a paragraph holds its ID as plain text.

| Case | Expectation |
| --- | --- |
| The rename | The anchor keeps the MID; the `ID` row, the text of every inbound relation link and of every mention whose text was the old ID carry the new ID; other mention text and the plain text are unchanged; the project gains no diagnostic |
| `get`, `item validate` and `relation add` given the old ID afterwards | Fail with `unknown-item` |
| The rename stopped before its `ID` row, and after it, in the order of [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) | As [VER-INTERRUPTED-EDITS](#01M7F74J3SE8HJ3S6ZNG3G2DYJ) requires |
| Code markers that name the old ID, from 0.4.0-alpha.5 (mara-yhxq5x) | As [VER-CODE-EDITS](../code/verification.mara.md#01M7JKT47QXAAP2VG6S3P1Y7XB) requires |

---

<a id="01M7F74J164RHHPWYRT5W0JQV9"></a>

### Keep every byte a write does not need

| Type | Verification |
| --- | --- |
| ID | VER-PRESERVED-BYTES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-MINIMAL-REWRITE](writes.mara.md#01M7BF6HHH01T5XJNAF7DM433R) |

Rust integration tests run each operation that writes documents, `item create`, `item update`, `item move`, `item rename`, `item delete`, `relation add`, `relation remove`, `project fix` and `project convert`, with the real CLI on disposable projects. Their documents have CRLF line endings in one file and LF in another, a byte order mark, front matter, irregular table padding in the items each operation edits, `*` and `_` emphasis, trailing spaces, no final newline, and file modes 0600 and 0644.

| Case | Expectation |
| --- | --- |
| Each operation | Every file it changes differs from its former contents only in the spans the operation requires; the bytes before and after each span, the other rows of an edited table included, are identical |
| Each operation | A changed file keeps its mode; every other file keeps its name, bytes and mtime |
| `project convert` | Narrative outside directive blocks, `[[ID]]` and `[[MID]]` mentions and inline typed relations is byte-identical |

---

<a id="01M7F74J2P251MSZKVK231NBR5"></a>

### Refuse an edit without writing

| Type | Verification |
| --- | --- |
| ID | VER-REFUSED-EDITS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-REFUSED-EDIT-CHANGES-NOTHING](writes.mara.md#01M7BF6HHTMM6D6QAF0BHFBCE6) |

Rust integration tests run each operation that [VER-PRESERVED-BYTES](#01M7F74J164RHHPWYRT5W0JQV9) lists, and `project init`, with the real CLI on disposable projects. After each refusal, no file in the project tree has been created, deleted or modified: names, bytes and mtimes are unchanged.

| Case | Expectation |
| --- | --- |
| Each refusal that the operation's own Verification item lists | Refused; nothing changed |
| A project whose configuration or schema has an error, for each operation but `project init` and `project convert`; an unknown or ambiguous ID; an undeclared option | Refused with `invalid-project`, `unknown-item`, `ambiguous-item` or `invalid-argument`; nothing changed |
| A file the edit will write, changed by another program while the writer waits at the pause before step 2 ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)): the only file of a single-file edit, and the first and a later file of a multi-file edit; for `item create` into a new document, that document created | Refused: nothing replaced, every file unchanged except the one the test changed, and no temporary file left. Establishes the check of step 2 for every target |
| The first or only file the edit replaces, changed by another program while the writer waits at its pause before that file's second check; for `item create` into a new document, that document created | Refused: nothing replaced, that file keeps the other program's contents, every other file is unchanged and no temporary file is left. Establishes the check before the first rename, single-file edits included |

---

<a id="01M7F74J3SE8HJ3S6ZNG3G2DYJ"></a>

### Stop each multi-file edit between its files

| Type | Verification |
| --- | --- |
| ID | VER-INTERRUPTED-EDITS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](writes.mara.md#01M7BF6HJ3758Y06V61901ZSKK) |

Rust integration tests run `item move`, `item rename`, `project fix` and `project convert` with the real CLI, on Linux, on disposable projects whose items have relations and mentions across three directories. Each run stops the operation at one file it writes, F, in one of four ways, and the runs cover every F and every way. For the first two ways, F is a document or code file padded with narrative or comments so that its new contents are larger than those of every other file the operation writes. The last two use the pause before a file's second check ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) in the debug binary, and let every other stop go on at once. The test learns the order in which the operation replaces files from the paths those pauses report in an uninterrupted run; for a move and a rename it is that of [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17), and for a conversion that of [DES-CONVERSION-MAPPING](../migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM).

| Stop | How the test causes it | Files afterwards, and what the case establishes |
| --- | --- | --- |
| Killed | `setrlimit` in the child before `exec` sets `RLIMIT_FSIZE` between the largest other new contents and F's new contents; `SIGXFSZ` keeps its default action, so the kernel kills the process while it writes F's temporary file | F holds its former contents, or none if the operation creates it, and every other file its former or its new contents, byte for byte; for a move or rename, exactly the files before F in the order hold their new contents |
| Temporary write fails | The same limit, with `SIGXFSZ` ignored before `exec`: the write fails with `EFBIG` | As when killed; exit status 1 with a diagnostic; no temporary file is left |
| Changed at its pause point | The writer waits at F's pause point, F not being the first file it replaces; the test changes F, then removes `paused` | The files before F hold their new contents, F the test's contents and every later file its former contents; exit status 1 with a diagnostic that names F. Establishes the check before F's rename; [VER-REFUSED-EDITS](#01M7F74J2P251MSZKVK231NBR5) covers the first file |
| Rename fails | The writer waits at F's pause point, after writing F's temporary file; the test removes write permission from F's directory, removes `paused`, and restores the permission after the process exits | As when killed; exit status 1 with a diagnostic. Establishes a failed rename |

- A change after F's second check and before its rename stays undetected, as [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) states; no case claims more.
- No temporary file, left or removed, is ever read as an input.
- A conversion writes its report file first: "Killed" and "Temporary write fails" stop it there with any limit below the report's size.
- "Rename fails" needs a user without `CAP_DAC_OVERRIDE`; run as root, the test fails and says so.

| Case | Expectation |
| --- | --- |
| Each stop | Every item is still in some file with its MID, rows and body |
| Each stop of a move, rename or fix | `project validate` reports every duplicate item, stale link path, stale link text and unresolved code marker left, and the project has no other new diagnostic |
| A conversion stopped while it writes the report file | No document is converted and no report file is in place |
| A conversion stopped after a document whose conversion leaves something out | The report file holds that omission |
| Each stop of a conversion, then a rerun | Before the rerun, the project is a 0.3.0 project that every other operation refuses with `legacy-project`; after it, the project's files, the report file included, are those of an uninterrupted conversion, and the rerun returns every entry of that report |
| A move stopped while the item is in both files, then rerun; the item's own links are relative | The move completes and the project gains no diagnostic |
| A rename stopped before its `ID` row, then rerun | The rename completes |
| A rename stopped after its `ID` row | Every mention whose text was the old ID carries the new ID; the only leftover is stale relation link text, which `project validate` reports and `project fix` rewrites ([VER-FIX-CASES](#01M7F74HZVJPEQRJWABH3NTDW5)) |
| From 0.4.0-alpha.5 (mara-yhxq5x), a rename whose markers are in two code files, stopped at the second code file, then rerun | Before the rerun, the first file's markers name the new ID and are each `unresolved-marker`, and the second file's name the old ID; after it, every marker names the new ID and the project has no diagnostic |
| The same rename stopped after its `ID` row | Every marker names the new ID and resolves |

---

<a id="01M7F74J4HC0VT3K1YJDDNKJ0P"></a>

### Run Mara writers at the same time

| Type | Verification |
| --- | --- |
| ID | VER-CONCURRENT-WRITERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-WRITERS-SERIALIZED](writes.mara.md#01M7CXC2N8EZYVHGY7T2GPPB3X) |

Rust integration tests start real Mara writers on one disposable project at the same time.

| Case | Expectation |
| --- | --- |
| Ten `item create` processes, with distinct titles, started at once on one document | Every item a process reports as created is in the document; every other process reports a refusal; the project gains no diagnostic |
| A write started while the test holds the project's write lock as a Mara writer does; the test changes a file the write targets, then releases the lock | No file changes while the lock is held; then the write is applied to the changed contents or refused |
| A CLI write and an MCP `item_create` call at once | As for two CLI writes |
| The daemons of two builds, copies of the binary at two paths ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)), from 0.4.0-alpha.4 (mara-0mxw21), and an embedded Node host, from 0.4.0-alpha.6 (mara-xzfeyj), each writing at the same time as the CLI | As for two CLI writes |

---

<a id="01M7F74HZVJPEQRJWABH3NTDW5"></a>

### Repair only what fix may repair

| Type | Verification |
| --- | --- |
| ID | VER-FIX-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-ONE-FIX-OPERATION](index.mara.md#01M7BF6GNPRRHNP2TRX7MKZRRH) |

Rust integration tests run `project fix` with the real CLI on disposable projects.

| Case | Expectation |
| --- | --- |
| A heading and a table with a `Type` row but no anchor | An anchor with a new MID is added, and the item loads |
| A bare-ID relation value that names one item | The canonical link to that item |
| A stale link path in a relation and in a mention, and stale relation link text | The current path and ID; mention text unchanged |
| A `missing-separator`, an anchor that lacks only its blank line, and a setext heading in an item's heading place | A `---` with a blank line on each side before the anchor; a blank line after the anchor; an ATX heading of the same level |
| Items written without `---` and anchor, one after a title and an introduction and one right after another item, with a bare-ID relation value; tables with `Type` rows that make no item in the body before them | One fix adds the `---` and an anchor before each heading and the link; the item before keeps its body up to the `---`, mentions included; the other tables stay body text, unreported |
| After another item without `---`: an item whose ID is malformed, one without an `ID` row, and one after a custom HTML anchor | Reported before the fix, and an update of the item before keeps them; one fix adds the `---`, and an anchor except after the HTML; the ID and HTML problems stay reported, and a second fix finds nothing |
| A body or new item that holds a heading followed by an item's table | Refused, naming `missing-separator` |
| After each of these | No diagnostic of the repaired kind is left |
| An anchor without a heading or `Type` row, an item without an `ID` row, a bare ID that names no item, and a broken link | Unchanged and still reported: no heading, type or ID is supplied |
| Two copies of one item | Both are kept |
| Stale link paths and text in several files, as a move or rename stopped by [VER-INTERRUPTED-EDITS](#01M7F74J3SE8HJ3S6ZNG3G2DYJ) leaves them | No stale link path or text is left |
| A valid project | Names, bytes and mtimes unchanged |
| Each other operation that [VER-PRESERVED-BYTES](#01M7F74J164RHHPWYRT5W0JQV9) lists, on a project with the problems above outside what it edits | Those problems stay as they were |
