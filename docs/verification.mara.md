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
| An operation that reads no code ([DES-CODE-READERS](code.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) and runs no program | 0 subprocesses, 0 code scans |

Cold rows of the operations that 0.4.0-alpha.2 adds hold in every host and run through the real CLI from 0.4.0-alpha.2. The checks of [DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) read and hash a file again only when [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) would:

| Situation | Expectation |
| --- | --- |
| A write by a direct CLI call, every file it writes last changed at least 2 s before the write starts | The counts of a cold request plus 1 parse per distinct content written, before the renames ([ADR-READ-BACK-EVERY-EDIT](authoring.mara.md#01M7FH6HEWNYQ66F5KE912BQSM)), and nothing more: no read, hash or snapshot build before or after the renames, and no parse after them ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) steps 3 and 4) |
| `prime`, `project init` | 0 file reads, 0 hashes, 0 parses, 0 snapshot builds |
| `project convert`, on the small fixed 0.3.0 input of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) | No read or hash of `.git` or of a file that the 0.3.0 configuration does not declare, except the project's `.gitignore` files, and a report file or `.mara/schema.toml` that an earlier run left ([DES-CONVERSION-MAPPING](migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM)) |

Cold rows of the operations that 0.4.0-alpha.3 adds hold in every host and run through the real CLI from 0.4.0-alpha.3. A program run is one evaluation by Soufflé ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)); in CLI direct mode each is one subprocess spawn, and no other process is spawned:

| Situation | Expectation |
| --- | --- |
| `project validate` of a project that declares programs | 1 program run per declared program, whether an invariant uses it or not |
| `query execute`, `query view`, `query gate` | 1 program run |
| `query report` | 1 program run, in provenance mode |
| `query list`; a request refused for a program the parser refuses or for a missing or wrong input | 0 program runs |

Warm rows need a host that keeps state between requests ([REQ-WORK-FOLLOWS-CHANGE](performance.mara.md#01M7BF6H3A6YQ43351V9YSV3SQ)) and run from 0.4.0-alpha.4, at 1×, through the real CLI against a daemon the test starts with its own runtime directory ([VER-DAEMON-START](verification-daemon.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)). Each row is its own test and starts settled: the last change came at least 2 s before the last request, so every input was last hashed by a check at least 2 s after its mtime and ctime ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). A row's counts are those of its write, or else of the request sent after its situation, background re-derivation included ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). The rows for one document edited and for the request after a write also run with that request sent 2 s later, after the watcher re-derived, and give the same counts:

| Situation | Expectation |
| --- | --- |
| The same read request again, inputs unchanged | 0 file reads, 0 hashes, 0 parses, 0 snapshot builds, 0 code scans, 0 subprocess spawns, 0 check steps, 0 program runs |
| Another request, inputs unchanged | 0 file reads, 0 hashes, 0 parses, 0 snapshot builds |
| A request whose evaluations an earlier request ran with the same keys ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)), such as `query view` after `project validate` | 0 program runs |
| An input last hashed by a check less than 2 s after its mtime or ctime, unchanged | That input read and hashed again, and no other; 0 parses, 0 snapshot builds |
| An input touched, its contents unchanged | That input read and hashed again, and no other; 0 parses, 0 snapshot builds, 0 program runs |
| One document edited in place, or saved by renaming a new file over it | That document read and hashed, and no other input; 1 parse, 1 snapshot build, 0 code scans |
| A document added; one removed; one renamed | 1 parse, 0 parses and 0 parses; 1 snapshot build each |
| The schema or the configuration edited, the same files remaining inputs | 0 parses, 1 snapshot build |
| A write that gives each file it writes contents no file of the session had before | 1 parse per distinct content written, 1 snapshot build; no parse of another file |
| A write that gives a file the contents of another input, such as `item delete` of the only item that tells two documents apart | No parse for that file |
| A write that is the session's first request, every file it writes last changed at least 2 s before | The counts of a cold request, no read or hash before the renames, the parses of the two write rows above, made by the read-back before the renames, then their snapshot build |
| The request after a write, its files written less than 2 s before | Those files read and hashed again, and no other input; 0 parses, 0 snapshot builds |
| One code file edited, then a request that reads code, from 0.4.0-alpha.5 (mara-779tms) | 1 code scan, 0 document parses |
| One program file edited, then `project validate` | 1 program run, of that program; 0 parses, 0 snapshot builds |
| `project validate` or `query gate` after a document edit | 1 program run per program it evaluates |
| `query gate` with other supplied inputs, nothing else changed | 1 program run |
| A file that is no input changed, in a directory that holds inputs or in ignored build output; a `.git` change; a failed request; 5 s without a request | No invalidation: the next request counts as on unchanged inputs |
| A second project's session opened and its inputs edited | The first project's next request counts as on unchanged inputs |
| The project reached through a symbolic link to its root, after requests through its real path | As the same request again: one session per canonical root ([DES-DAEMON-SESSIONS](daemon.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)) |
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

On every pull request, Rust integration tests time each operation except `project convert` ([ADR-NO-CONVERTER-SCALING](decisions-runtime.mara.md#01M7F9N0JK7BSWE98V9VTCGBV0)) on a release build at each pull-request corpus size (median of runs). They fail when growth per doubling exceeds the limit. From 0.4.0-alpha.4 they also time a daemon the test starts, by the same rule: settled warm requests and requests after one document edit, with the daemon's CPU time measured as [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) says.

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

The daemon case sends every document operation but `project init` and `project convert`, failing requests and writes included, from the CLI and from `mara mcp` to a daemon the test starts ([VER-DAEMON-START](verification-daemon.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)), after an earlier request opened the session. It pairs generated MIDs as [VER-CLI-MCP-PARITY](verification-hosts.mara.md#01M7F74J5Z4KX9F3HN78T1WB85) does, and requires the same files after each write.

The Node library and browser cases are modules run by `node --test`, Node's own runner, which a Rust integration test of the `mara` crate starts, so that `cargo test` runs them. The browser is Playwright's Chromium, driven by Playwright's library from those modules. A case needs only what its backend uses, and a missing piece fails only the cases that need it, with a message that names it: Node 24 for every case; for a browser workspace, Chromium, the page server and the engine's WASM module, and Soufflé's when the case runs a program; for an embedded Node case, the built addon; for a Node case through the daemon, the addon and a daemon; for Chromium through the daemon, Chromium, the page server and a daemon started with `--http`. The test creates what its cases use: a daemon with the runtime directory of [VER-DAEMON-START](verification-daemon.mara.md#01M7JKW7JWB2RANYKNE6BHYM26), started with `--http` for the Chromium cases and allowing only the test's projects and page origin; an HTTP server on a loopback port the operating system assigns, which serves the test's pages and the package's files; and a browser launched with a new profile in the test's temporary directory, never a persistent context. At the end it closes the browser and the server and stops any daemon it started; then no process whose environment or command line names the test's temporary directory runs. Every Node and browser case of a Verification item runs in this setup. Each case compares with CLI direct mode on the same files, pairing MIDs as above:

| Case | Expectation |
| --- | --- |
| The daemon case's requests on the 1× corpus of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA), `project init` in an empty directory and `project convert` of that harness's 0.3.0 input, through the Node client embedded and through the daemon | Each result, or failure's code, message and diagnostics, as in CLI direct mode; the same files after each write |
| The same from Chromium through the daemon | The same, but `project init` and `project convert` fail with `unavailable-operation` ([VER-UNAVAILABLE-NOT-PARTIAL](verification-browser.mara.md#01M7JQJKGBJ6VC7X844R6HZZGS)) |
| The same in a browser workspace supplied with every file of the project but `.git` | As in the row above; after each write, the supplied files with the exported change set applied by the test are the files of CLI direct mode |
| A browser workspace supplied with the 1× corpus: every query and view of `engineering`, `query gate` with the corpus's gate inputs, and `query report` of its failing row | The rows of CLI direct mode: Soufflé's WASM and native builds give the same outputs |

Architecture checks run from 0.4.0-alpha.1. They guard [DES-ENGINE-BOUNDARY](engine.mara.md#01M7BF6GY13YX9PD76M0TQG8YK) and do not establish the requirement without the parity cases:

- The local checks build the engine crate for `wasm32`.
- Review of each change to the engine crate finds no filesystem, process, thread, clock or network access in its code or dependencies.

---

<a id="01M7CT60NQK9TAH7JD15HCR2XE"></a>

### Exercise the engineering template's acceptance cases

| Type | Verification |
| --- | --- |
| ID | VER-TEMPLATE-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-TEMPLATE-ACCEPTANCE-RULES](template.mara.md#01M7CT60KVJ75RV9GPP3SEF0WX) |
| Verifies | [REQ-TEMPLATE-GATES-PROVE-READINESS](template-readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

Rust integration tests run the real CLI on disposable projects initialized from `engineering`.

| Case | Expectation |
| --- | --- |
| Accepted requirement with only a draft basis, or only a draft verification | Validation error; the same items as drafts are valid |
| Requirement with two accepted verifications, one passing and the other failing or absent from a complete input | `verified` fails |
| Result for another candidate, or for an older revision of the verification or of the requirement | Does not count: `verification-status` is `pending` with a complete input and `unavailable` with an incomplete one; `verified` does not pass |
| Validation result for another context, or for an older revision of the validation or of its need or goal | Does not count: `validation-status` is `pending` with a complete input and `unavailable` with an incomplete one; `release-ready` does not pass |
| For one verification and requirement, and for one validation and need, counting results `pass` and `fail`, `pass` and `error`, and `pass` and `inconclusive` | `verification-status` and `validation-status` give the worse result; `verified` and `release-ready` fail |
| Risk on a parent requirement, on its need or on that need's goal | Relevant to the child requirement; a sibling requirement is not judged |
| HIGH risk with a mitigation | Blocks until it is reassessed or an accepted decision accepts it |
| Named validation whose result is absent from a complete input, or is not a pass | `release-ready` fails; a validation that is not named does not block |
| Validation of two needs with a pass for one and, for the other, a failing result or none in a complete input | `release-ready` fails |
| A failure and an unavailable fact on the same item | Failed |
| Item whose only missing fact is a risk level, an implementation link while code is unread, or a result absent from an incomplete input | Unavailable, never passed |
| Accepted requirement without an accepted verification | `verified` fails |
| A failing row of each template invariant and gate | Has an explanation |
| `C supersedes B supersedes A` after B retires | Valid |
| Requirement implemented only through a design, from 0.4.0-alpha.5 (mara-yhxq5x) | Counts when the design is accepted, not when it is draft |
| From 0.4.0-alpha.5, a requirement that an `implements` marker names, with an accepted Verification whose result passes and a named Validation of its Need that passes | `implemented` has the marker's row; `verified` and `release-ready` pass |
| From 0.4.0-alpha.5, markers naming a Design and a Verification | `implemented` has a row for each |
| From 0.4.0-alpha.5, a requirement that no marker names, every code file read; the same in a project that declares no code language | `verified` and `release-ready` fail ([DES-CODE-READERS](code.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) |
| From 0.4.0-alpha.5, a requirement that no marker names while a code file cannot be read | `verified` and `release-ready` unavailable |
| A view and a gate over the same items | Report the same facts |
| One project per rule of [DES-TEMPLATE-ACCEPTANCE](template.mara.md#01M7CT60M2EQ33WTZFNG600D4H) and of [DES-TEMPLATE-RELATION-LIFECYCLE](template.mara.md#01M7CT60M96Q91T5KXNQ1G58PP), with an accepted item that breaks it | A validation error of that rule's invariant at the item; valid when the item is a draft |
| A risk in each of the 25 cells of [DES-TEMPLATE-RISK-LEVEL](template.mara.md#01M7CT60MFPF2Y871JJDZP5K0T) | `risk-level` gives that cell's level |
| `requirement-basis` of a requirement with a draft and an accepted basis | Both rows, with their statuses |
| Each view of [DES-TEMPLATE-VIEWS](template-readiness.mara.md#01M7CT60NHPG3GY0V1F0MZ5QF0) for a requirement without an accepted verification, one whose result is absent from an incomplete input, and one with a failing result | A row for each; the cells for missing, unavailable and failed differ |
