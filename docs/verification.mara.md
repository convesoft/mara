# Verification

---

<a id="01M7BF6HGG6ET88HS641Y5676G"></a>

### Assert exact work counters

| Type | Verification |
| --- | --- |
| ID | VER-WORK-COUNTERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Verifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

On every pull request, Rust integration tests run the real CLI and daemon on a generated corpus and assert the work counters ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) exactly. They also assert the bounds of [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) on built-in checks and program runs.

Cold rows hold in every host and run through the real CLI from 0.4.0-alpha.1:

| Situation | Expectation |
| --- | --- |
| Cold request, two documents with the same contents included | For each input it reads, 1 file read and 1 hash; 1 parse per distinct document content ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)); 1 snapshot build |
| `.git` and undeclared files | 0 file reads, 0 hashes |
| An operation that reads no code ([DES-CODE-READERS](code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) and runs no program | 0 subprocesses, 0 code scans |

Cold rows of the operations that 0.4.0-alpha.2 adds hold in every host and run through the real CLI from 0.4.0-alpha.2. The checks of [DES-WRITE-PROCEDURE](authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) read and hash a file again only when [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) would:

| Situation | Expectation |
| --- | --- |
| A write by a direct CLI call, every file it writes last changed at least 2 s before the write starts | The counts of a cold request plus 1 parse per distinct content written that no document of the snapshot has, before the renames ([ADR-READ-BACK-EVERY-EDIT](authoring/index.mara.md#01M7FH6HEWNYQ66F5KE912BQSM)), and nothing more: no read, hash or snapshot build before or after the renames, and no parse after them ([DES-WRITE-PROCEDURE](authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) steps 3 and 4) |
| `prime`, `project init` | 0 file reads, 0 hashes, 0 parses, 0 snapshot builds |
| `project convert`, on the small fixed 0.3.0 input of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) | No read or hash of `.git` or of a file that the 0.3.0 configuration does not declare, except the project's `.gitignore` files, and a report file or `.mara/schema.toml` that an earlier run left ([DES-CONVERSION-MAPPING](migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM)) |

Cold rows of the operations that 0.4.0-alpha.3 adds hold in every host and run through the real CLI from 0.4.0-alpha.3. A program run is one evaluation by Soufflé ([DES-RULE-EXECUTION](rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)); in CLI direct mode each is one subprocess spawn, and no other process is spawned:

| Situation | Expectation |
| --- | --- |
| `project validate` of a project that declares programs | 1 program run per declared program that an entry names or that no other declared file includes ([DES-RULE-EXECUTION](rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)), whether an invariant uses it or not |
| `query execute`, `query view`, `query gate` | 1 program run |
| `query report` | 1 program run, in provenance mode |
| `query list`; a request refused for a program the parser refuses or for a missing or wrong input | 0 program runs |

Cold rows of the operations that 0.4.0-alpha.4 adds run through the real CLI from 0.4.0-alpha.4, with a runtime directory of the test ([VER-DAEMON-START](daemon/verification.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)):

| Situation | Expectation |
| --- | --- |
| `daemon status`, `daemon start`, `daemon stop`, each with and without a daemon running | 0 of every count |

Warm rows need a host that keeps state between requests ([REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ)) and run from 0.4.0-alpha.4, at 1×, through the real CLI against a daemon the test starts with its own runtime directory ([VER-DAEMON-START](daemon/verification.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)). Each row is its own test and starts settled: the last change came at least 2 s before the last request, so every input was last hashed by a check at least 2 s after its mtime and ctime ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). A row's counts are those of its write, or else of the request sent after its situation, background re-derivation included ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). The rows for one document edited and for the request after a write also run with that request sent 2 s later, after the watcher re-derived: the write row gives the same counts, and the edited-document row the counts it states:

| Situation | Expectation |
| --- | --- |
| The same read request again, inputs unchanged | 0 file reads but one of each file that `--input` names, 0 hashes, 0 parses, 0 snapshot builds, 0 code scans, 0 subprocess spawns, 0 check steps, 0 program runs |
| Another request, inputs unchanged | 0 file reads but one of each file that `--input` names, and one read and hash of a code file without markers that `get` reads and whose contents no earlier `get` kept ([DES-DAEMON-SESSIONS](daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)); otherwise 0 hashes, 0 parses, 0 snapshot builds |
| A request whose evaluations an earlier request ran with the same keys ([DES-RULE-EXECUTION](rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)), such as `query execute` of an invariant's program after `project validate`, while the session still keeps their outcomes ([DES-DAEMON-SESSIONS](daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)) | 0 program runs |
| An input last hashed by a check less than 2 s after its mtime or ctime, unchanged | That input read and hashed again once: by the request's check, or by a derivation of the watcher that checked it before the request, 2 s or more after the change, which settles it; a derivation that checks it sooner leaves it unread ([DES-DAEMON-SESSIONS](daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)); no other input; 0 parses, 0 snapshot builds |
| An input touched, its contents unchanged | That input read and hashed once by the request's check, and once more by the watcher's derivation when that checked it before the request, less than 2 s after the touch; once, by that derivation alone, when it checked it 2 s or more after the touch, which settles it; no other input; 0 parses, 0 snapshot builds, 0 program runs |
| One document edited in place, or saved by renaming a new file over it | That document read and hashed once, by the request, when no derivation checked it before the request; twice when the watcher's derivation checked it first, less than 2 s after the edit, as it does in the run sent 2 s later: by that derivation and by the request's check, which settles it ([DES-DAEMON-SESSIONS](daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)); once, by that derivation alone, when it checked it first 2 s or more after the edit; no other input; 1 parse, 1 snapshot build, 0 code scans. Later requests run no program whose evaluation key the edit leaves as it was while the session keeps its outcome; a stop at a limit, a fault or an evicted outcome runs again |
| A document added; one removed; one renamed | 1 parse, 0 parses and 0 parses; 1 snapshot build each |
| The schema or the configuration edited, the same files remaining inputs | 0 parses, 1 snapshot build |
| A write that gives each file it writes contents no file of the session had before | 1 parse per distinct content written, 1 snapshot build; no parse of another file |
| A write that gives a file the contents of another input, such as `item delete` of the only item that tells two documents apart | No parse for that file |
| A write that is the session's first request, every file it writes last changed at least 2 s before | The counts of a cold request, no read or hash before the renames, the parses of the two write rows above, made by the read-back before the renames, then their snapshot build |
| The request after a write, sent at once, or 2 s later after the watcher's derivation | In both, the written files read and hashed again once, by the request's check, and no other input: the watcher's derivation reads none of them; 0 parses, 0 snapshot builds. A derivation that checks them before the request, 2 s or more after the write, reads them once instead, which settles them, and the request reads none |
| One code file edited, then a request that reads code, from 0.4.0-alpha.5 (mara-779tms) | 1 code scan, 0 document parses |
| One program file that no other program includes edited, then `project validate` | 1 program run, of that program; 0 parses, 0 snapshot builds |
| `project validate` or `query gate` after a document edit | 1 program run per program it evaluates with the snapshot's facts; none for a program run against empty facts whose outcome the session keeps, and none at all when the session keeps the outcomes of the edited snapshot's key, as after a document restored byte for byte |
| `query gate` with other supplied inputs, nothing else changed | 1 program run |
| A file that is no input changed, in a directory that holds inputs or in ignored build output; a `.git` change; a failed request; 5 s without a request | No invalidation: the next request counts as on unchanged inputs |
| A second project's session opened and its inputs edited | The first project's next request counts as on unchanged inputs |
| The project reached through a symbolic link to its root, after requests through its real path | As the same request again: one session per canonical root ([DES-DAEMON-SESSIONS](daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)) |
| One `mara mcp` call, after the same request from the CLI | The server's stats line: as the same request again |

From 0.4.0-alpha.6, warm rows also run in a browser workspace on the 1× corpus, in the setup of [VER-HOST-COMPARISON](#01M7D3NAQHP8HX0X0GCZMXD1Q9), read through the workspace's debug stats option ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). Contents the caller supplies or writes are hashed by the engine and are no file reads:

| Situation | Expectation |
| --- | --- |
| The same read request again, nothing written (mara-jptfbq) | 0 file reads, 0 hashes, 0 parses, 0 snapshot builds, 0 check steps, 0 program runs |
| One document written by the caller, then a read (mara-jptfbq) | 1 hash, of that document; 1 parse; 1 snapshot build |
| `query view` after `project validate`, nothing written (mara-xsm8w6) | 0 program runs |
| One program written by the caller, then `project validate` (mara-xsm8w6) | 1 program run, of that program; 0 parses, 0 snapshot builds |

---

<a id="01M7BF6HGRNQ5SJVNP89G1CCBW"></a>

### Measure growth per doubling

| Type | Verification |
| --- | --- |
| ID | VER-SCALING |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |

On every pull request, Rust integration tests time each operation except `project convert` ([ADR-NO-CONVERTER-SCALING](decisions.mara.md#01M7F9N0JK7BSWE98V9VTCGBV0)) on a release build at each pull-request corpus size (median of runs). They fail when growth per doubling exceeds the limit. From 0.4.0-alpha.4 they also time a daemon the test starts, by the same rule: settled warm requests and requests after one document edit, with the daemon's CPU time measured as [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) says.

---

<a id="01M7BF6HH06WEJ67P04H22BM66"></a>

### Check time budgets nightly

| Type | Verification |
| --- | --- |
| ID | VER-TIME-BUDGETS |
| Status | Draft |
| Methods | Test |
| Verifies | [REQ-TIME-BUDGETS](performance.mara.md#01M7BF6H47MJ8H16R3B4S7750T) |
| Verifies | [REQ-READS-DURING-VALIDATION](performance.mara.md#01M7BF6H3T3589901T1AE0ADHM) |

Nightly on a reference machine, `hyperfine` times release builds end to end against the budgets in each mode: CLI direct, daemon cold, daemon warm, daemon after a watched edit, and browser-local WASM once it exists. One run issues a read during a long validation of the same project.

---

<a id="01M7D3NAQHP8HX0X0GCZMXD1Q9"></a>

### Compare hosts on the same project

| Type | Verification |
| --- | --- |
| ID | VER-HOST-COMPARISON |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-SAME-RESULT-ON-EVERY-HOST](engine.mara.md#01M7BF6GXHZNABX9R7NKPNGJNM) |

Rust integration tests run each operation on the same disposable project in CLI direct mode and in each other host that supports it, and require identical JSON results. Each parity case starts when its host exists:

| Host compared with CLI direct mode | From |
| --- | --- |
| Daemon | 0.4.0-alpha.4 |
| Node library, embedded and through the daemon | 0.4.0-alpha.6 |
| Browser, through the daemon and local WASM | 0.4.0-alpha.6 |

The daemon case sends every document operation but `project init` and `project convert`, failing requests and writes included, from the CLI and from `mara mcp` to a daemon the test starts ([VER-DAEMON-START](daemon/verification.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)), after an earlier request opened the session. Its project declares a code language and holds markers, so that the operations that read code ([DES-CODE-READERS](code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) read them, and `item rename` rewrites them. It pairs generated MIDs as [VER-CLI-MCP-PARITY](hosts/verification.mara.md#01M7F74J5Z4KX9F3HN78T1WB85) does, and requires the same files after each write. It fails when the tool list of `mara mcp` holds a document operation, but those two, that it did not send. That daemon answered each request: `served` of `mara daemon status` rose by one across it ([VER-DAEMON-START](daemon/verification.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)), and by none across a request whose arguments the client rejects before it sends it, which `mara mcp` answers as it does in direct mode, naming the parameter ([DES-MCP-SERVER](hosts/index.mara.md#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)). At the end that daemon is the only daemon of the test.

The Node library and browser cases are modules run by `node --test`, Node's own runner, which a Rust integration test of the `mara` crate starts, so that `cargo test` runs them. The browser is Playwright's Chromium, driven by Playwright's library from those modules. A case needs only what its backend uses, and a missing piece fails only the cases that need it, with a message that names it: Node 24 for every case; for a browser workspace, Chromium, the page server and the engine's WASM module, and Soufflé's when the case runs a program; for an embedded Node case, the built addon; for a Node case through the daemon, the addon and a daemon; for Chromium through the daemon, Chromium, the page server and a daemon started with `--http`. The test creates what its cases use: a daemon with the runtime directory of [VER-DAEMON-START](daemon/verification.mara.md#01M7JKW7JWB2RANYKNE6BHYM26), started with `--http` for the Chromium cases and allowing only the test's projects and page origin; an HTTP server on a loopback port the operating system assigns, which serves the test's pages and the package's files; and a browser launched with a new profile in the test's temporary directory, never a persistent context. At the end it closes the browser and the server and stops any daemon it started; then no process whose environment or command line names the test's temporary directory runs. Every Node and browser case of a Verification item runs in this setup. Each case compares with CLI direct mode on the same files, pairing MIDs as above:

| Case | Expectation |
| --- | --- |
| The daemon case's requests on the 1× corpus of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA), `project init` in an empty directory and `project convert` of that harness's 0.3.0 input, through the Node client embedded and through the daemon | Each result, or failure's code, message and diagnostics, as in CLI direct mode; the same files after each write |
| The same from Chromium through the daemon | The same, but `project init` and `project convert` fail with `unavailable-operation` ([VER-UNAVAILABLE-NOT-PARTIAL](#01M7JQJKGBJ6VC7X844R6HZZGS)) |
| The same in a browser workspace supplied with every file of the project but `.git` | As in the row above; after each write, the supplied files with the exported change set applied by the test are the files of CLI direct mode |
| A browser workspace supplied with the 1× corpus: every query and view of `engineering`, `query gate` with the corpus's gate inputs, and `query report` of its failing row | The rows of CLI direct mode: Soufflé's WASM and native builds give the same outputs |

Architecture checks run from 0.4.0-alpha.1. They guard [DES-ENGINE-BOUNDARY](engine.mara.md#01M7BF6GY13YX9PD76M0TQG8YK) and do not establish the requirement without the parity cases:

- The local checks build the engine crate for `wasm32`.
- Review of each change to the engine crate finds no filesystem, process, thread, clock or network access in its code or dependencies.

---

<a id="01M7D3NAQQ599DWC895A7TG3TC"></a>

### Check documents against their GitHub rendering

| Type | Verification |
| --- | --- |
| ID | VER-GITHUB-RENDERING |
| Status | Accepted |
| Methods | Test |
| Methods | Analysis |
| Methods | Inspection |
| Verifies | [REQ-SOURCE-IS-PUBLISHED-FORM](format.mara.md#01M7BF6GYWE2DT7WSXQH2QPNPA) |

Rust integration tests run the real CLI on disposable projects.

| Case | Method | Expectation |
| --- | --- | --- |
| A document with `***` and `___` breaks and a `---` line inside a fenced code block | Test | Regions break at the `***` and `___` lines only |
| A document that starts with YAML front matter | Test | No diagnostic |
| Same-file, cross-directory and table-cell relation links and mentions in a project that validates without diagnostics | Test | Each link's path, resolved from the linking file, names the file that holds the anchor of its MID |
| GitHub's handling of those links | Analysis | They have the forms that [EVD-GITHUB-FRAGMENT-MATCHING](identity.mara.md#01M7BF6H2MHAVK1Z5HCXR1V289) found GitHub to navigate to their target |
| Items and links written by `item create`, `relation add` and `item move`, in a new document too, from 0.4.0-alpha.2 | Test | As for the hand-written links; no written file starts with `---` |
| The CLI's operations | Inspection | None writes documents in another format |

---

<a id="01M7D3NAQYGXQ5YG1YMRV12SKS"></a>

### Resolve hand-edited items by MID

| Type | Verification |
| --- | --- |
| ID | VER-MID-RESOLUTION |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-STABLE-IDENTITY](identity.mara.md#01M7BF6H1AN6G9GP3DV62342WJ) |

Rust integration tests run the real CLI on a disposable project in which another file links to an item by a relation and by a mention. The tests then change the item's title and ID and move it to another file by hand, leaving the links as they were.

| Case | Expectation |
| --- | --- |
| `get` of the MID | The item, with its new title, ID and location |
| `relation list` of the item | The incoming relation and the mention, at their unchanged source locations |
| The same three changes made with `item update`, `item rename` and `item move`, from 0.4.0-alpha.2 | The same results, and the anchor keeps the MID |

---

<a id="01M7D3NAR43MYRDF7YKMBDSJ10"></a>

### Select the project among nested projects

| Type | Verification |
| --- | --- |
| ID | VER-NESTED-PROJECTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-ONE-PROJECT-PER-OPERATION](project.mara.md#01M7BF6H8ND7YSXFMWCM0KXAAY) |

Rust integration tests run each CLI operation on a disposable project that holds a second project in a subdirectory. The include patterns of both match every `*.mara.md` file below their roots, so the outer project's patterns also match the inner project's documents. An operation uses only a project when no result holds an item, narrative unit or diagnostic from another project's files, and the file-read counter ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) counts only that project's inputs.

| Case | Expectation |
| --- | --- |
| Run from a directory inside the inner project | Uses only the inner project: none of the outer project's files is read |
| Run from the outer project, outside the inner one | Uses only the outer project: none of the inner project's files is read |
| `--project` naming one project, run from inside the other | Uses only the named project |
| Run from the outer project after the inner project's `.mara/config.toml` is replaced by `.mara/project.toml` | None of the inner project's documents is read |
| Run from the outer project when a subdirectory holds a `config.toml` outside any `.mara/` directory | The subdirectory's documents are inputs of the outer project |
| Run outside every project | Fails with a diagnostic and parses no document |
| A directory with `.mara/project.toml` and no `.mara/config.toml` | Refused with a diagnostic that names the conversion |
| A write run from the outer project that names a document path below the inner project's root, from 0.4.0-alpha.2 | Refused; no file of either project changes |

---

<a id="01M7D3NARATT16K40DZCCFCX3H"></a>

### Validate against the project's own schema

| Type | Verification |
| --- | --- |
| ID | VER-CUSTOM-SCHEMA |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-SCHEMA-DEFINES-VOCABULARY](project.mara.md#01M7BF6H97RDY6XY07F5SC4777) |

Rust integration tests run the real CLI on a disposable project whose schema declares types, fields, values, statuses and relations of its own, and none of the `engineering` template's.

| Case | Expectation |
| --- | --- |
| Items that use only the declared vocabulary | Valid |
| An item of type `Requirement`; a row keyed by an `engineering` field or relation; a field value or `Status` outside the declared lists | An error each |
| An item of a type without fields, with only `Type`, `ID` and `Status` rows | Valid |
| A schema edit that adds the rejected field value to its list | The same document becomes valid |

---

<a id="01M7D3NARGT43PEVX6SWPN60CZ"></a>

### Report every seeded problem

| Type | Verification |
| --- | --- |
| ID | VER-SEEDED-PROBLEMS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-REPORT-EVERY-PROBLEM](validation.mara.md#01M7BF6HEVWZ2490K493GDP8KC) |
| Verifies | [REQ-VALIDITY-COVERS-WHOLE-PROJECT](validation.mara.md#01M7BF6HF8R0Q5BK349ESZ2PS7) |

Rust integration tests run the real CLI on a disposable project seeded, several to a file, with one instance of each failure the built-in checks define ([DES-SCHEMA-CHECKS](validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)), an unknown configuration setting and an invalid schema declaration.

| Case | Expectation |
| --- | --- |
| `project validate` | Each seeded problem exactly once, with its code, severity, path and line, and nothing else |
| `schema validate` | The schema's problem, and no document's |
| `item validate` of an item whose ID another file repeats and whose relation link has a stale path | Both problems and the configuration's, and none of another item |
| `project validate` with a page of one diagnostic, or filtered to a path without errors | Invalid, with the project's total error count and a non-zero exit status |
| The same project with every problem removed | Valid, no diagnostic and exit status 0 |

---

<a id="01M7D3NARPYEP9XHY86H015Y4Z"></a>

### Read an item with invalid rows

| Type | Verification |
| --- | --- |
| ID | VER-INVALID-ROWS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-INVALID-ROWS-REPORTED](validation.mara.md#01M7BF6HFZVTK5PC7W60FPND94) |

Rust integration tests run the real CLI on a disposable project with an item that has an undeclared key, a value outside its field's allowed set and a row that repeats another.

| Case | Expectation |
| --- | --- |
| `project validate` | One diagnostic for each of the three rows |
| `get` of the item | The item with all its rows in source order, the invalid ones included |

---

<a id="01M7D3NARW1657P2G2P9BWXSWR"></a>

### Run every read on a broken project

| Type | Verification |
| --- | --- |
| ID | VER-UNCHANGED-SOURCES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-READS-NEVER-WRITE](validation.mara.md#01M7BF6HFM0MQ2NH7RA7S00B8K) |

Rust integration tests run every validation and read operation of the CLI, failing requests included, on a disposable project with a missing anchor, a bare-ID relation value, stale link paths and text, invalid rows and CRLF line endings. After each run, no file in the project tree has been created, deleted or modified: names, bytes and mtimes are unchanged.

---

<a id="01M7F74J51BE6J8BQSH842BXWV"></a>

### Initialize a project from each template

| Type | Verification |
| --- | --- |
| ID | VER-PROJECT-INIT |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-INIT](project.mara.md#01M7BF6H8Z02SNKXD2XPGVD87M) |

Rust integration tests run `project init` with the real CLI in disposable directories (`crates/mara/tests/init.rs`). For `schema list` and `schema get`, the CLI output must match the engine's output for the template. The docs-checks test (`crates/docs-checks/tests/template.rs`) compares that engine output with the vocabulary page.

| Case | Expectation |
| --- | --- |
| No template named, in the working directory and in a named one | The template's files are written there; the output lists exactly the files created; `project validate` reports no diagnostic |
| `schema list` and `schema get` of that project | Every type and relation of [DES-ENGINEERING-VOCABULARY](template/vocabulary.mara.md#01M7BF6HH8CA2B9TQJ9DT32WDG), with its ID prefix, description, fields, allowed values, target types and constraints, and nothing else |
| Items of that project with each of the statuses `Draft`, `Accepted` and `Retired`, and with another status | Valid; invalid |
| `empty` named | A configuration and a schema that declares no type and no relation; `project validate` reports no diagnostic |
| A directory in which one of the files to write exists | Refused, naming that file; no file or directory is created |
| The programs of `engineering` | Written under `.mara/rules/` and listed; `project validate` compiles them without a diagnostic; `query list` lists the template's invariants, queries, gates and three views ([DES-TEMPLATE-VIEWS](template/readiness.mara.md#01M7CT60NHPG3GY0V1F0MZ5QF0)) |

---

<a id="01M7F74J5E4771N36RX8H50X3P"></a>

### Convert the 0.3.0 corpus and one fixture per mapping row

| Type | Verification |
| --- | --- |
| ID | VER-CONVERSION |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CONVERT-DIRECTIVE-CORPUS](migration.mara.md#01M7BF6H30RYJTH5Z2YERBJ690) |

Rust integration tests run `project convert` with the real CLI on disposable copies of this repository's 0.3.0 project (`docs/` and `.mara/` at tag `v0.3.0`) and of fixtures.

| Case | Expectation |
| --- | --- |
| The 0.3.0 project | Every MID, ID and field value of its items is in the converted project; every relation that is not symmetric and leads from an item to an item, in a metadata row or inline, under the forward or the inverse name, is one row on its source item; `project validate` reports only the items reported for the author to complete; everything left out is reported; `.mara/conversion-report.json` holds the entries the result returns |
| A fixture for each row of [DES-CONVERSION-MAPPING](migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM), among them relations written from the target, thematic breaks and setext headings in bodies, `integer`, `number` and `boolean` fields, and empty bodies | Converted as the row says, and reported where it says so |
| Include patterns `docs/{a,b}.mara.md` and `docs/[ab].mara.md` | Kept as written, and reported |
| An item without a status, with a status named for the run | That status |
| Two values of one field, or two statuses, that become the same label | Nothing written; each pair reported |
| The 0.3.0 project's conversion stopped while it writes the report file and after each document, one of them a document whose conversion leaves something out, then rerun | As [VER-INTERRUPTED-EDITS](authoring/verification.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) requires |
| After a stop, a `[[ID]]` mention added by hand to a converted document from which a symmetric relation row was dropped, then a rerun | The returned report and the report file still hold that omission, once |
| After a stop, a rerun with one unconverted document unchanged and another edited so that one of its omissions moves to another line | Each unchanged entry once; for the moved omission, the recorded entry and the rerun's entry at its new line |
| A report file left by an earlier run that is a symbolic link, is not JSON of that object, or has `format_version` 2 | Refused, naming the file; nothing written |
| Every other operation on a 0.3.0 project | Refused, as [VER-NESTED-PROJECTS](verification.mara.md#01M7D3NAR43MYRDF7YKMBDSJ10) requires |
| A `:::mara` block in a converted project's document | Read as narrative: it makes no item |

---

<a id="01M7F74J6E2V10YX6RYENGHC1P"></a>

### Compare both renderings of the agent guide

| Type | Verification |
| --- | --- |
| ID | VER-AGENT-GUIDE |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-AGENT-GUIDE](agents.mara.md#01M7BGRHWB2MGXEY5G41YP8E0G) |

Rust integration tests run `mara prime` and read the instructions of the real stdio MCP server, each started inside and outside disposable projects.

| Case | Expectation |
| --- | --- |
| Inside a project, found from the working directory or named by `--project` | Both open with the same fixed heading and have the sections find and read, author, repair after hand edits, link code, and validate; operations appear as commands in `prime` and as tools in the MCP instructions |
| Outside every project | Both say how to initialize a project and name no other operation or tool |
| A 0.3.0 project | Both say how to convert it and name no other operation or tool |
| Each operation, option, tool and parameter that either rendering names | The CLI declares it or the MCP server lists it, compared as [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) describes |
| Each operation that works in a project | The full guide names it, as a command in `prime` and as a tool in the MCP instructions |
| Two projects whose schemas differ | The same guide |
| A project whose configuration has an error | The full guide |
| A server started with and one started without `--project` | The same instructions but for the sentence on the project; each names only parameters its tools list |
| The MCP instructions | At most 3,800 characters |

---

<a id="01M7JQJKGBJ6VC7X844R6HZZGS"></a>

### Report what a host cannot do as unavailable

| Type | Verification |
| --- | --- |
| ID | VER-UNAVAILABLE-NOT-PARTIAL |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-UNAVAILABLE-NOT-PARTIAL](engine.mara.md#01M7BF6GXSJVVASJ4C8ZGHM909) |

In the setup of [VER-BROWSER-WORKSPACE](hosts/verification.mara.md#01M7JQJKGBSCMPWE7CGNPP7PNB). On the native hosts, code that cannot be read is checked by [VER-CODE-FACTS](code/verification.mara.md#01M7JKT47QVCRS95RJC08VA021) and [VER-TEMPLATE-CASES](template/index.mara.md#01M7CT60NQK9TAH7JD15HCR2XE), and a grammar that cannot be loaded by [VER-GRAMMAR-MARKERS](code/verification.mara.md#01M7JKT47QR2NYFK59JRV8EV7G).

| Case | Expectation |
| --- | --- |
| `project init` of each template, and `project convert` of the 0.3.0 input of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA), in a workspace and through the daemon from Chromium | Fail with `unavailable-operation` naming the operation; the next export is empty, and the daemon's project is byte-identical |
| The fixtures of [VER-GRAMMAR-MARKERS](code/verification.mara.md#01M7JKT47QR2NYFK59JRV8EV7G), with both grammars pinned, in a workspace: each operation that reads code, those that read it only for a program from mara-xsm8w6 | The result of CLI direct mode on the same files with both grammars removed from the cache, but for each `unavailable-grammar` message, which says the browser cannot load grammars; `project validate` is invalid |
| The setup of [VER-PROGRAM-FAULTS](rules/verification.mara.md#01M7HZYKB4R632Q63YWS8CY1FJ) with a program that passes, in a workspace whose Soufflé module the test's server answers with status 404, and in a second run with a truncated module | A fault without a line at the configuration line of each declared program; `project validate` is invalid, `query execute` and `query view` fail, and the gate and both items are unavailable |

---

<a id="01M82C8WTK2PAA1SBEWCAJ4ND8"></a>

### Install the packed packages on a machine without a toolchain

| Type | Verification |
| --- | --- |
| ID | VER-INSTALL-WITHOUT-TOOLCHAIN |
| Status | Accepted |
| Methods | Test |
| Methods | Analysis |
| Verifies | [REQ-RUN-WITHOUT-TOOLCHAIN](distribution.mara.md#01M7BF6GWPMWDV4VT1BA7NEZYN) |

The install verification runs on each platform of the version ([DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8)): in the release workflow's Build job on that platform's build image, before anything is published, on the packed dispatcher and native package, and in its Registry check job on the version installed from npm ([DES-RELEASE-WORKFLOW](distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF)); and locally on Linux x64, on packed tarballs. It installs the packages with `npm install --ignore-scripts` into an empty directory under its temporary directory, packed tarballs without fetching a package of the version from the registry, through npm's offline mode with an empty cache of the verification's own; installing, unpacking and reading the binaries' symbols run with the caller's environment. It stages links to `node` and `npm` in a private directory under its temporary directory. It then runs each case through `node_modules/.bin/mara`, each operation with `--no-daemon` but in the daemon case, with an environment that holds only `PATH`, naming only that private directory, `HOME`, a new directory under its temporary directory, and `XDG_RUNTIME_DIR`, a new directory of mode 0700 whose path is short enough for the daemon's socket ([DES-DAEMON-LIFECYCLE](daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)). A case that fails names its command and output.

| Case | Method | Expectation |
| --- | --- | --- |
| `cargo`, `rustc`, `cc`, `c++`, `g++`, `clang` and `souffle` looked up in that `PATH` | Test | None is found; otherwise the verification fails naming it before any other case |
| `mara --version` | Test | Prints `mara <version>` and exits 0 |
| `project init` in an empty directory, `item create` of an Accepted requirement without a basis, then `project validate --debug-stats` | Test | Exits 1; among the diagnostics, an error of the invariant `requirement-has-basis` at that item; the stats line counts at least one program run, which only the package's `souffle` can have run |
| `mara mcp` started in that project, sent an `initialize` request, then its standard input closed | Test | Answers with `serverInfo` named `mara` and of version `<version>`, and exits 0 within 5 s |
| `daemon start`, `project validate`, `daemon status`, `daemon stop` in that project | Test | The validation's result as in direct mode; `served` of the status is 1; after the stop no process of the verification runs |
| The dispatcher installed without its optional dependencies, then `mara --version` | Test | Exits 1; stderr names the native package and says to reinstall with optional dependencies |
| The same, with `NODE_PATH` naming the `node_modules` folder of the installation that holds the native package | Test | As without `NODE_PATH`: Node would find the native package there, and the dispatcher refuses it |
| The dispatcher run with Node reporting glibc 2.34, then reporting none, through a preload module of the verification that replaces `process.report.getReport` | Test | Exits 1 without starting `mara`; stderr names 2.35 and 2.34, then that glibc 2.35 or later is required |
| The installed dispatcher's `docs/`, walked without following a symbolic link | Test | Exactly the files that Git tracks under `docs/` at the checkout's HEAD, each a regular file with its blob's contents. It runs in the Build jobs and in the Registry check, whose checkouts are the recorded commit ([DES-RELEASE-WORKFLOW](distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF)) |
| Each packed tarball's file list | Test | Exactly the files that DES-PACKAGES lists for that package, the two binaries with mode 0755, and the dispatcher's `docs/` with the contents of those files |
| The `GLIBC_` symbol versions that `bin/mara` and `bin/souffle` of the native package need | Analysis | None exceeds 2.35. Binaries built on a newer glibc, as locally on a current distribution, fail it, as they would fail on glibc 2.35 |
| After every case | Test | Nothing that the case started runs: a child that the case has not reaped, or a daemon by the process ID that its status reports while its executable lies in the verification's temporary directory, is ended with `SIGTERM`, awaited, and fails the case. Any other process whose executable lies there fails the case and is reported, never signalled |
