# Code link verification

---

<a id="01M7JKT47QKGBA9KQYFWDE21Q9"></a>

### Link code through markers at the basic level

| Type | Verification |
| --- | --- |
| ID | VER-CODE-MARKERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CODE-MARKER-LINKS](code.mara.md#01M7BF6GPD85NN3TCFJE7RCKWB) |

Rust integration tests run the real CLI on a disposable project initialized from `engineering`. Its configuration declares three languages at the basic level ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)): Rust in `src/**/*.rs` with the comment prefixes `//`, `///` and `*`, Python in `scripts/**/*.py` with `#`, and TypeScript in `web/**/*.ts` with `//` and `*`. Markers use the `implements` relation of the engineering vocabulary, whose inverse is `implemented_by`.

| Case | Expectation |
| --- | --- |
| Markers as developers write them: above a Rust function, indented inside its body, in a `///` doc comment, in a Python script with a shebang, as a ` * @mara implements <ID>` line of a TypeScript JSDoc block, after code on the same line (`check(); // @mara implements <ID>`), in a Rust file with CRLF line endings, and two on consecutive lines naming two items | `relation list` of each named item: one incoming `implemented_by` connection per marker, whose neighbour is the code location `<path>:<line>` of the marker and whose source is the same; `project validate` valid |
| `relation list <path>:<line>` of a marker's line, and `relation list <path>` of its file | That marker's outgoing `implements` connection to its item; every marker of the file, in line order |
| `relation list` of an item narrowed to `implemented_by`, and to another relation | Only its code links; none of them |
| `get <path>:<line>` of a marker's line, and `get <path>` | The file from that line to its end; the whole file |
| `// @mara implements` without an ID, `// @mara implements REQ-A REQ-B`, and `// @mara implemnts REQ-A` | `invalid-marker` at each line; no connection |
| A marker whose ID no item has: a typo, the ID of an item whose `ID` row was changed by hand, and the ID of an item removed by hand | `unresolved-marker` at each marker; invalid, exit status 1; `relation list` of the item under its new ID has no code link |
| `implements` naming a Need | `disallowed-target` at the marker |
| A string literal `" // @mara implements <ID>"`, its prefix after a space | Read as a comment start at the basic level: `invalid-marker`, since the quote follows the ID ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) |
| `@mara implements <ID>` after other words in a comment, in a string literal right after its quote (`"// @mara implements <ID>"`), in a `.txt` file under `src/`, in a `.rs` file that a `.gitignore` ignores, in a `.rs` file below a nested project's root, and in a symbolic link to a `.rs` file | No connection and no diagnostic |
| The same marker in two files, as in a copied file | Two connections, each with its own file and line; no diagnostic |
| A marker naming an ID that two items have | One connection to each item; no diagnostic besides the items' `duplicate-id` |
| A Rust file that is not UTF-8, with a Latin-1 comment and a marker | The marker's connection, as in a UTF-8 file |
| Between two `relation list` requests of an item, a marker added, one moved to another line and one removed | The second request returns the current markers at their current lines, and no other |
| A `relation list` cursor of an item, used after a code file changed; a `search` cursor, used after the same change | Rejected as stale; accepted |
| The same files in a project that declares no code language | No code link and no marker diagnostic |
| After every case | Every document keeps its name, bytes and mtime: no code endpoint is written into a document |
| Each case through `mara mcp`, and through the daemon | The JSON results of CLI direct mode ([VER-CLI-MCP-PARITY](verification-hosts.mara.md#01M7F74J5Z4KX9F3HN78T1WB85), [VER-HOST-COMPARISON](verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9)) |

---

<a id="01M7JKT47QR2NYFK59JRV8EV7G"></a>

### Own markers by declaration with a grammar

| Type | Verification |
| --- | --- |
| ID | VER-GRAMMAR-MARKERS |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-CODE-MARKER-LINKS](code.mara.md#01M7BF6GPD85NN3TCFJE7RCKWB) |
| Verifies | [REQ-ONLY-CODE-ADD-DOWNLOADS](code.mara.md#01M7BF6GPR0B36WAK7ESQH41WM) |

In the setup of [VER-CODE-MARKERS](#01M7JKT47QKGBA9KQYFWDE21Q9), the test's fixtures hold the official release files, the `.wasm` and `queries/tags.scm`, of the tree-sitter grammars for Rust and Python, at the versions of the built-in catalog. The test installs them in the per-user cache under its own `HOME` and pins them in the configuration, as `code add` stores and pins them ([DES-GRAMMAR-ASSETS](code.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)); mara-s9gak4 defines that place and the pin.

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
| Verifies | [REQ-ONLY-CODE-ADD-DOWNLOADS](code.mara.md#01M7BF6GPR0B36WAK7ESQH41WM) |

Rust integration tests run the real CLI with `HOME` and the per-user cache inside the test's temporary directory. The test serves the grammar fixtures of [VER-GRAMMAR-MARKERS](#01M7JKT47QR2NYFK59JRV8EV7G) over HTTP on a loopback port as a given repository ([DES-GRAMMAR-ASSETS](code.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)), in the layout that mara-h0hxa3 defines, and counts the connections it accepts.

| Case | Expectation |
| --- | --- |
| `code add rust` from that repository | The files are in the cache; the configuration pins version, checksum and repository, and no other project file changes; markers in Rust files then have declaration endpoints |
| `code add rust` in a copy of that project with an empty cache, the server now serving other bytes | Refused, naming the checksum mismatch; the cache and the configuration unchanged |
| A cached file changed after `code add` | `unavailable-grammar`; file-and-line endpoints; 0 connections |
| Every other operation of [DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS), as the cases of the performance harness run it, with the pinned grammar absent from the cache | 0 connections; `unavailable-grammar` from each operation that reads code |
| A given repository `http://` on a host that is not a loopback address, or `ftp://` | Rejected with `invalid-argument`; nothing written |
| A language the catalog lacks, without a repository | Refused; nothing written |
| The configuration changed by another program at the pause before step 2 ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) | Refused with `changed-file`; the configuration keeps that program's contents |
| An ignored test, run with `--ignored` before each release: each catalog language added from its real repository | The fetched files match the catalog's checksums, and that language's markers get declaration endpoints |

Inspection: review of each change to `code add` or to Mara's dependencies finds network access only in `code add` and in libraries that only it calls.
