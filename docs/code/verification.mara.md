# Code link verification

---

<a id="01M7JKT47QKGBA9KQYFWDE21Q9"></a>

### Link code through markers at the basic level

| Type | Verification |
| --- | --- |
| ID | VER-CODE-MARKERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CODE-MARKER-LINKS](index.mara.md#01M7BF6GPD85NN3TCFJE7RCKWB) |

Rust integration tests run the real CLI on a disposable project initialized from `engineering`. Its configuration declares three languages at the basic level ([DES-CODE-MARKERS](index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)): Rust in `src/**/*.rs` with the comment prefixes `//`, `///` and `*`, Python in `scripts/**/*.py` with `#`, and TypeScript in `web/**/*.ts` with `//` and `*`. Markers use the `implements` relation of the engineering vocabulary, whose inverse is `implemented_by`.

| Case | Expectation |
| --- | --- |
| Markers as developers write them: above a Rust function, indented inside its body, in a `///` doc comment, in a Python script with a shebang, as a ` * @mara implements <ID>` line of a TypeScript JSDoc block, after code on the same line (`check(); // @mara implements <ID>`), in a Rust file with CRLF line endings, and two on consecutive lines naming two items | `relation list` of each named item: one incoming `implemented_by` connection per marker, whose neighbour is the code location `<path>:<line>` of the marker and whose source is the same; `project validate` valid |
| `relation list <path>:<line>` of a marker's line, and `relation list <path>` of its file | That marker's outgoing `implements` connection to its item; every marker of the file, in line order |
| `relation list` of an item narrowed to `implemented_by`, and to another relation | Only its code links; none of them |
| `get <path>:<line>` of a marker's line, and `get <path>` | The file from that line to its end; the whole file |
| `get <path>` and `get <path>:<line>` of a Rust file under a `src/.gitignore` whose pattern `[z-a].rs` the `ignore` crate refuses | Fails with `unknown-item`, carrying `unsupported-gitignore` at `src/.gitignore` line 1 ([DES-CODE-READERS](index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) |
| `// @mara implements` without an ID, `// @mara implements REQ-A REQ-B`, and `// @mara implemnts REQ-A` | `invalid-marker` at each line; no connection |
| A marker whose ID no item has: a typo, the ID of an item whose `ID` row was changed by hand, and the ID of an item removed by hand | `unresolved-marker` at each marker; invalid, exit status 1; `relation list` of the item under its new ID has no code link |
| `implements` naming a Need | `disallowed-target` at the marker |
| A string literal `" // @mara implements <ID>"`, its prefix after a space | Read as a comment start at the basic level: `invalid-marker`, since the quote follows the ID ([DES-CODE-MARKERS](index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) |
| `@mara implements <ID>` after other words in a comment, in a string literal right after its quote (`"// @mara implements <ID>"`), in a `.txt` file under `src/`, in a `.rs` file that a `.gitignore` ignores, in a `.rs` file below a nested project's root, and in a symbolic link to a `.rs` file | No connection and no diagnostic |
| The same marker in two files, as in a copied file | Two connections, each with its own file and line; no diagnostic |
| A marker naming an ID that two items have | One connection to each item; no diagnostic besides the items' `duplicate-id` |
| A Rust file that is not UTF-8, with a Latin-1 comment and a marker | The marker's connection, as in a UTF-8 file |
| Between two `relation list` requests of an item, a marker added, one moved to another line and one removed | The second request returns the current markers at their current lines, and no other |
| A `relation list` cursor of an item, used after a code file changed; a `search` cursor, used after the same change | Rejected as stale; accepted |
| The same files in a project that declares no code language | No code link and no marker diagnostic |
| After every case | Every document keeps its name, bytes and mtime: no code endpoint is written into a document |
| Each case through `mara mcp`, and through the daemon | The JSON results of CLI direct mode ([VER-CLI-MCP-PARITY](../hosts/verification.mara.md#01M7F74J5Z4KX9F3HN78T1WB85), [VER-HOST-COMPARISON](../verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9)) |

---

<a id="01M7JKT47QR2NYFK59JRV8EV7G"></a>

### Own markers by declaration with a grammar

| Type | Verification |
| --- | --- |
| ID | VER-GRAMMAR-MARKERS |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-CODE-MARKER-LINKS](index.mara.md#01M7BF6GPD85NN3TCFJE7RCKWB) |
| Verifies | [REQ-ONLY-CODE-ADD-DOWNLOADS](index.mara.md#01M7BF6GPR0B36WAK7ESQH41WM) |

In the setup of [VER-CODE-MARKERS](#01M7JKT47QKGBA9KQYFWDE21Q9), the test's fixtures hold the official release files, the `.wasm` and `queries/tags.scm`, of the tree-sitter grammars for Rust 0.24.2 and Python 0.25.0. The test installs them in the per-user cache under its own `HOME` and pins them in the configuration, as `code add` stores and pins them ([DES-GRAMMAR-ASSETS](index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)).

| Case | Expectation |
| --- | --- |
| A marker in the comments before a Rust function, a method of an `impl` block and a struct, with a doc comment and a `#[test]` attribute between | Owned by that declaration: `relation list` gives its name and its first line as the location's line, and the marker's line as the source |
| A marker inside a function's body; one at the top of a file, before any declaration | Owned by that function; by the file, at line 1 without a declaration name |
| Python markers before a `def`, before a `class` and inside a method | Owned by that function, class and method |
| Markers in `//` and `///` comments and as ` * @mara` lines inside a `/* */` block in Rust, and in `#` comments in Python | Each is a marker: comment node kinds `line_comment`, `block_comment` and `comment` |
| A marker-like line inside a Rust raw string that spans lines, and a string literal `" // @mara implements <ID>"` | No connection and no diagnostic; at the basic level, a connection for the first and `invalid-marker` for the second |
| A marker after code on its line inside a function's body, and after `struct S;` | Owned by that function; by `S` |
| `get <path>:<line>` of a line inside a function | That function, from its first line to its last |
| `relation list <path>:<line>` of a line inside a declaration that owns two markers | Both markers' connections |
| The grammar removed from the cache; a cached `.wasm` replaced by a truncated copy pinned with its own checksum | Every operation that reads code reports `unavailable-grammar` at the configuration line that pins it, and gives that language's markers file-and-line endpoints |
| Each case through `mara mcp`, and through the daemon | The JSON results of CLI direct mode |
| In a debug build, a grammar whose load `MARA_TEST_GRAMMAR_LOAD_STALL` keeps from ending, with `MARA_TEST_GRAMMAR_LOAD_MS` as its limit: three `relation list` calls to one `mara mcp` | Each reports `unavailable-grammar` and gives basic-level endpoints; only the first waits for the load's time, and the process keeps the one grammar thread it started |
| In a debug build, a daemon's request loading a grammar whose tags query is invalid, the load held by `MARA_TEST_GRAMMAR_LOAD_STALL` until the test lets it go and recorded by `MARA_TEST_GRAMMAR_EVENTS`: a request of a second project for the same grammar, a `get` of a code location in a third project whose language pins another grammar, then a later request for the first grammar | The `get` returns that grammar's declaration while both requests for the first grammar still wait; those report the load's failure as `unavailable-grammar` after one load, and the later request loads again |

Inspection: review of mara-s9gak4 finds that the grammar's ABI version is compared with the range the runtime supports before the grammar is used, and that a mismatch is handled as a module that cannot be loaded. No released grammar's ABI lies outside that range, so no test can load one.

---

<a id="01M7JKT47QF8VK4K8SZ5GP5EV3"></a>

### Download grammars only through `code add`

| Type | Verification |
| --- | --- |
| ID | VER-CODE-ADD |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-ONLY-CODE-ADD-DOWNLOADS](index.mara.md#01M7BF6GPR0B36WAK7ESQH41WM) |

Rust integration tests run the real CLI with `HOME` and the per-user cache inside the test's temporary directory. The test serves the grammar fixtures of [VER-GRAMMAR-MARKERS](#01M7JKT47QR2NYFK59JRV8EV7G) over HTTP on a loopback port as a given repository, in its layout ([DES-GRAMMAR-ASSETS](index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)), and counts the connections it accepts. The catalog cases give the debug binary a catalog of that server's URLs through `MARA_TEST_GRAMMAR_CATALOG`.

| Case | Expectation |
| --- | --- |
| `code add rust` from that repository | The files are in the cache; the configuration pins version, checksum and repository, and no other project file changes; markers in Rust files then have declaration endpoints |
| `code add rust` in a copy of that project with an empty cache, the server now serving other bytes | Refused, naming the checksum mismatch; the cache and the configuration unchanged |
| A cached file changed after `code add` | `unavailable-grammar`; file-and-line endpoints; 0 connections |
| Every other operation of [DES-OPERATIONS](../hosts/index.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS), as the cases of the performance harness run it, through the CLI and each MCP tool, with the pinned grammar absent from an empty cache | 0 connections; the exit status of the case; `unavailable-grammar` in the result of each operation that reads code and returns diagnostics |
| A given repository `http://` on a host that is not a loopback address, or `ftp://` | Rejected with `invalid-argument`; nothing written |
| A language the catalog lacks, without a repository | Refused; nothing written |
| The built-in catalog | Each entry's version and checksum are those of a grammar fixture, and its URLs name that version's tag |
| The configuration changed by another program at the pause before step 2 ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) | Refused with `changed-file`; the configuration keeps that program's contents |
| An ignored test, run with `--ignored` before each release: each catalog language added from its real repository | The fetched files match the catalog's checksums, and that language's markers get declaration endpoints |

Inspection: review of each change to `code add` or to Mara's dependencies finds network access only in `code add` and in libraries that only it calls.

---

<a id="01M7JKT47QXAAP2VG6S3P1Y7XB"></a>

### Rewrite markers and nothing else in code

| Type | Verification |
| --- | --- |
| ID | VER-CODE-EDITS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-RENAME-ID](../authoring/index.mara.md#01M7BF6GMNEZ9PYCWYKQZGY9YN) |
| Verifies | [REQ-MINIMAL-REWRITE](../authoring/writes.mara.md#01M7BF6HHH01T5XJNAF7DM433R) |

Rust integration tests run `item rename REQ-LOGIN REQ-SIGN-IN` with the real CLI in the setup of [VER-CODE-MARKERS](#01M7JKT47QKGBA9KQYFWDE21Q9), once at the basic level and once with the grammars of [VER-GRAMMAR-MARKERS](#01M7JKT47QR2NYFK59JRV8EV7G) installed. Markers name `REQ-LOGIN` before declarations and after code on the same line, in Rust files of two directories, a Python script with a shebang and mode 0755, a TypeScript JSDoc block, a Rust file with CRLF line endings, a Rust file that is not UTF-8 and one without a final newline. Other items target `REQ-LOGIN` with relations and mentions.

| Case | Expectation |
| --- | --- |
| The rename | Every marker that named `REQ-LOGIN` names `REQ-SIGN-IN`. Each code file differs from its former contents only in those IDs' bytes and keeps its mode; the result's `files` lists the code files before the item's document; `relation list REQ-SIGN-IN` returns every code link; the project gains no diagnostic |
| `REQ-LOGIN` in code outside a marker: in a string literal and in a comment that is no marker; a marker naming `REQ-LOGIN-AUDIT` | Unchanged |
| With grammars, Rust raw strings holding code examples with marker-like lines that name `REQ-LOGIN` | Unchanged: only the ID bytes of actual markers change |
| A declared code file that holds a marker naming `REQ-LOGIN` and cannot be read, the test running as a user without `CAP_DAC_OVERRIDE` | Fails with `io-error` naming that file, before writing anything; every file unchanged |
| Markers naming `REQ-LOGIN` in the files that [VER-CODE-MARKERS](#01M7JKT47QKGBA9KQYFWDE21Q9) shows to be no input: the `.txt` file, the ignored file, the nested project's file and the symbolic link | Names, bytes and mtimes unchanged, the link's target included |
| A code file changed by another program at the pause before step 2, and at its pause before its second check, as the first and as a later file the rename replaces | As [VER-REFUSED-EDITS](../authoring/verification.mara.md#01M7F74J2P251MSZKVK231NBR5) and [VER-INTERRUPTED-EDITS](../authoring/verification.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) require for that file |
| The rename through `mara mcp`, and through the daemon | The same files and the CLI's JSON result |

---

<a id="01M7JKT47QVCRS95RJC08VA021"></a>

### Give programs the code links

| Type | Verification |
| --- | --- |
| ID | VER-CODE-FACTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](../rules/index.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |
| Verifies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](../rules/index.mara.md#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

In the setup of [VER-PROGRAM-CONSUMERS](../rules/verification.mara.md#01M7HZYK9DYQDY8XFKWZES1ZB6), the configuration also declares Rust at the basic level, and in a second run at the grammar level with the fixture of [VER-GRAMMAR-MARKERS](#01M7JKT47QR2NYFK59JRV8EV7G). A program copies the code-link relation of [DES-FACT-BRIDGE](../rules/index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) into a consumed relation headed by the target's MID, and `source_complete` as [VER-FACT-ROWS](../rules/verification.mara.md#01M7HZYK9ZXDEW7JNRV16GG1D5) does.

| Case | Expectation |
| --- | --- |
| Markers for two items at each level, one of them whose relation does not lead from `code` to its target's type | One row per marker with the values `relation list` gives: relation, target MID, the location's path and line, the marker's line, and at the grammar level the declaration name |
| An unresolved marker and an invalid one; at the grammar level, a marker-like line in a string | No row |
| At the grammar level, a grammar that is missing from the cache, and one dropped while the request scans a later file | Basic-level rows for every file of its language, as `relation list` gives them in the same conditions; `unavailable-grammar` among the request's diagnostics |
| `query report` of an invariant row that rests on a code link, at each level | A `code_link` leaf at the code file and the marker's line ([DES-RULE-EXECUTION](../rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)) |
| Every code file read, at either level, also with a grammar that fell back to the basic level; a project that declares no code language | `source_complete("code")` ([DES-CODE-READERS](index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) |
| A code file without read permission, the test running as a user without `CAP_DAC_OVERRIDE` | No `source_complete("code")`; the readable files' rows |
| A program that does not name the code-link relation, run by `query execute` | Its rows as without the code files, and no code file read ([VER-CODE-WORK-COUNTERS](#01M7JKT47QNN11F4ZG7SG03CG8)); `project validate` still scans code and reports marker diagnostics |

---

<a id="01M7JKT47QNN11F4ZG7SG03CG8"></a>

### Count the work of reading code

| Type | Verification |
| --- | --- |
| ID | VER-CODE-WORK-COUNTERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-WORK-FOLLOWS-CHANGE](../performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Verifies | [REQ-NEAR-LINEAR-SCALING](../performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

The `performance` integration test ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) asserts these rows exactly, from 0.4.0-alpha.5 (mara-yhxq5x), whose generated configuration declares Rust at the basic level, so that the corpus's code files carry links. The counters do not tell a code file from a document, so "as without code" means the counts of the same case on the corpus whose configuration declares no code language. No counter counts directory listings or watches: a listing of a directory shows only when it reads that directory's `.gitignore` or reports a problem there, so the rows below detect a listing only of directories that hold one.

Cold rows, in CLI direct mode:

| Situation | Expectation |
| --- | --- |
| `relation list`, `project validate`, `item rename`, `item delete`, and `query execute`, `query view`, `query gate` and `query report` of a program that uses code links, such as the `engineering` readiness gates | For each code file, 1 read and 1 hash; 1 code scan per distinct code file content ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)), so two code files with the same contents are scanned once |
| `item rename` that rewrites markers, every file it writes last changed at least 2 s before | The counts of the row above and of the write row of [VER-WORK-COUNTERS](../verification.mara.md#01M7BF6HGG6ET88HS641Y5676G); no code scan of the new contents |
| `get` of a code location, of a line or of the whole file, with a `.gitignore` in its directory, one in another code directory and a problem below its directory, at the basic and at the grammar level | 1 read and 1 hash of that code file and of each `.gitignore` of the directories on the way to it, and of no other code file or `.gitignore`; no diagnostic of another directory; 1 code scan at the grammar level, 0 at the basic level; 0 parses and 0 snapshot builds at either level |
| `get` of a path that a code include pattern matches and that names a directory holding a `.gitignore` | The counts of `get` of a path there that does not exist, and the same failure |
| `get` of a document's path, of a line in it, or of a path that no code include pattern matches, with a `.gitignore` and a problem in the code tree, at the basic and at the grammar level | The counts of `get` of the item by ID, and no diagnostic of code |
| Every other operation, `search` included, and the `query` operations of a program that does not use code links | As without code |
| The `query` operations above with the corpus's Rust grammar pinned and cached | The counts at the basic level: grammar files are not counted, and a program that does not use code links loads no grammar |
| Any operation in a project that declares no code language | 0 code scans |
| `code add` from the loopback repository of [VER-CODE-ADD](#01M7JKT47QF8VK4K8SZ5GP5EV3) | 0 parses, 0 code scans, 0 snapshot builds |

Warm rows, against the daemon, besides the code row of [VER-WORK-COUNTERS](../verification.mara.md#01M7BF6HGG6ET88HS641Y5676G):

| Situation | Expectation |
| --- | --- |
| A code file edited to the contents of another code file | 0 code scans |
| A code file added; one removed; one renamed, its contents unchanged | 1 code scan; 0 code scans; 1 read and 1 hash of the new path and 0 code scans; 0 parses in all three |
| `get` of a document's path, of a line in it, or of a path that no code include pattern matches, with a `.gitignore` and a problem in the code tree, at either level, the session holding the snapshot and the inputs settled | 0 of each count, and no diagnostic of code |
| Code files edited, then a request that reads no code | No read, hash or scan of a code file; the next request that reads code scans each edited file once |
| A document edited, then `relation list` | 0 code scans |
| A language's comment prefixes changed in the configuration, or its grammar added by `code add` | 1 code scan per distinct content of that language's files, none of another language's; 0 parses |
| `item rename` that rewrites markers | 1 code scan per distinct code content written, of no other code file |
| `get` of a code location, at either level, when the session holds no contents of that file as it is, also after the code file changed | 1 read and 1 hash of that code file, its contents as on disk, and of each `.gitignore` on the way to it of which the session holds no record as it is, also in a session that never listed code; 1 code scan at the grammar level, 0 at the basic level; 0 parses and 0 snapshot builds; the next request that reads documents parses none and builds no snapshot |
| The same `get` again, the code file, the directories above it and their `.gitignore` files unchanged since a check 2 s or more after their change, also in a session that never listed code, with the code tree of the cold row, and again after `project validate` and `item list` | 0 of every count at either level, and no diagnostic of another directory: the session keeps the listings and `.gitignore` files on the way to the file, its contents and its scan ([DES-DAEMON-SESSIONS](../daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)) |

Scaling: the hub project of [DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) also declares Rust and holds 10 × size Rust files of distinct contents with 100 markers each naming the hub item. `project validate`, `relation list` of the hub and `item rename` of the hub on it are counter and scaling cases: each counts 1 code scan per code file, and its growth per doubling stays within the limit of [REQ-NEAR-LINEAR-SCALING](../performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK).
