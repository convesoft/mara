# Code link use verification

---

<a id="01M7JKT47QXAAP2VG6S3P1Y7XB"></a>

### Rewrite markers and nothing else in code

| Type | Verification |
| --- | --- |
| ID | VER-CODE-EDITS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-RENAME-ID](authoring.mara.md#01M7BF6GMNEZ9PYCWYKQZGY9YN) |
| Verifies | [REQ-MINIMAL-REWRITE](writes.mara.md#01M7BF6HHH01T5XJNAF7DM433R) |

Rust integration tests run `item rename REQ-LOGIN REQ-SIGN-IN` with the real CLI in the setup of [VER-CODE-MARKERS](verification-code.mara.md#01M7JKT47QKGBA9KQYFWDE21Q9), once at the basic level and once with the grammars of [VER-GRAMMAR-MARKERS](verification-code.mara.md#01M7JKT47QR2NYFK59JRV8EV7G) installed. Markers name `REQ-LOGIN` before declarations and after code on the same line, in Rust files of two directories, a Python script with a shebang and mode 0755, a TypeScript JSDoc block, a Rust file with CRLF line endings, a Rust file that is not UTF-8 and one without a final newline. Other items target `REQ-LOGIN` with relations and mentions.

| Case | Expectation |
| --- | --- |
| The rename | Every marker that named `REQ-LOGIN` names `REQ-SIGN-IN`. Each code file differs from its former contents only in those IDs' bytes and keeps its mode; the result's `files` lists the code files before the item's document; `relation list REQ-SIGN-IN` returns every code link; the project gains no diagnostic |
| `REQ-LOGIN` in code outside a marker: in a string literal and in a comment that is no marker; a marker naming `REQ-LOGIN-AUDIT` | Unchanged |
| With grammars, Rust raw strings holding code examples with marker-like lines that name `REQ-LOGIN` | Unchanged: only the ID bytes of actual markers change |
| A declared code file that holds a marker naming `REQ-LOGIN` and cannot be read, the test running as a user without `CAP_DAC_OVERRIDE` | Fails with `io-error` naming that file, before writing anything; every file unchanged |
| Markers naming `REQ-LOGIN` in the files that [VER-CODE-MARKERS](verification-code.mara.md#01M7JKT47QKGBA9KQYFWDE21Q9) shows to be no input: the `.txt` file, the ignored file, the nested project's file and the symbolic link | Names, bytes and mtimes unchanged, the link's target included |
| A code file changed by another program at the pause before step 2, and at its pause before its second check, as the first and as a later file the rename replaces | As [VER-REFUSED-EDITS](verification-writes.mara.md#01M7F74J2P251MSZKVK231NBR5) and [VER-INTERRUPTED-EDITS](verification-writes.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) require for that file |
| The rename through `mara mcp`, and through the daemon | The same files and the CLI's JSON result |

---

<a id="01M7JKT47QVCRS95RJC08VA021"></a>

### Give programs the code links

| Type | Verification |
| --- | --- |
| ID | VER-CODE-FACTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](rules.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |
| Verifies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](rules.mara.md#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

In the setup of [VER-PROGRAM-CONSUMERS](verification-rules.mara.md#01M7HZYK9DYQDY8XFKWZES1ZB6), the configuration also declares Rust at the basic level, and in a second run at the grammar level with the fixture of [VER-GRAMMAR-MARKERS](verification-code.mara.md#01M7JKT47QR2NYFK59JRV8EV7G). A program copies the code-link relation of [DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) into a consumed relation headed by the target's MID, and `source_complete` as [VER-FACT-ROWS](verification-rules.mara.md#01M7HZYK9ZXDEW7JNRV16GG1D5) does.

| Case | Expectation |
| --- | --- |
| Markers for two items at each level, one of them whose relation does not lead from `code` to its target's type | One row per marker with the values `relation list` gives: relation, target MID, the location's path and line, the marker's line, and at the grammar level the declaration name |
| An unresolved marker and an invalid one | No row |
| Every code file read, at either level, also with a grammar that fell back to the basic level; a project that declares no code language | `source_complete("code")` ([DES-CODE-READERS](code.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) |
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
| Verifies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Verifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

The `performance` integration test ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) asserts these rows exactly, from 0.4.0-alpha.5 (mara-yhxq5x), whose generated configuration declares Rust at the basic level, so that the corpus's code files carry links. The counters do not tell a code file from a document, so "as without code" means the counts of the same case on the corpus whose configuration declares no code language.

Cold rows, in CLI direct mode:

| Situation | Expectation |
| --- | --- |
| `relation list`, `project validate`, `item rename`, `item delete`, and `query execute`, `query view`, `query gate` and `query report` of a program that uses code links, such as the `engineering` readiness gates | For each code file, 1 read and 1 hash; 1 code scan per distinct code file content ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)), so two code files with the same contents are scanned once |
| `item rename` that rewrites markers, every file it writes last changed at least 2 s before | The counts of the row above and of the write row of [VER-WORK-COUNTERS](verification.mara.md#01M7BF6HGG6ET88HS641Y5676G); no code scan of the new contents |
| `get` of a code location | 1 read and 1 hash of that code file and of no other; 1 code scan at the grammar level, 0 at the basic level |
| Every other operation, `search` included, and the `query` operations of a program that does not use code links | As without code |
| Any operation in a project that declares no code language | 0 code scans |
| `code add` from the loopback repository of [VER-CODE-ADD](verification-code.mara.md#01M7JKT47QF8VK4K8SZ5GP5EV3) | 0 parses, 0 code scans, 0 snapshot builds |

Warm rows, against the daemon, besides the code row of [VER-WORK-COUNTERS](verification.mara.md#01M7BF6HGG6ET88HS641Y5676G):

| Situation | Expectation |
| --- | --- |
| A code file edited to the contents of another code file | 0 code scans |
| A code file added; one removed | 1 code scan; 0 code scans; 0 parses in both |
| Code files edited, then a request that reads no code | No read, hash or scan of a code file; the next request that reads code scans each edited file once |
| A document edited, then `relation list` | 0 code scans |
| A language's comment prefixes changed in the configuration, or its grammar added by `code add` | 1 code scan per distinct content of that language's files, none of another language's; 0 parses |
| `item rename` that rewrites markers | 1 code scan per distinct code content written, of no other code file |

Scaling: the hub project of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) also declares Rust and holds 10 × size Rust files of distinct contents with 100 markers each naming the hub item. `project validate`, `relation list` of the hub and `item rename` of the hub on it are counter and scaling cases: each counts 1 code scan per code file, and its growth per doubling stays within the limit of [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK).
