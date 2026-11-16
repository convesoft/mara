# Node library and browser verification

---

<a id="01M7JQJKGB1770A965KZDW21TX"></a>

### Run Mara from Node

| Type | Verification |
| --- | --- |
| ID | VER-NODE-LIBRARY |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-NODE-LIBRARY](hosts.mara.md#01M7BF6H09NQCQTEZA4WEZAXEZ) |

In the setup of [VER-HOST-COMPARISON](verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9), which holds the Node parity cases, Node 24 runs ES modules of the test that import the package by its name; its daemon cases are in [VER-DAEMON-START](verification-daemon.mara.md#01M7JKW7JWB2RANYKNE6BHYM26). `tsc` is TypeScript's compiler.

| Case | Expectation |
| --- | --- |
| `tsc --strict --noEmit` of a module of the test that calls every document operation and reads every member of its result, with lines marked `// @ts-expect-error`: an operation that does not exist, an argument of the wrong type, a required argument left out, a result member the operation does not return. The same module passes each operation's parameter types and awaited result type to a recursive type of its own that does not compile when the type, or a member, array element or union member of it at any depth, is `any` (`0 extends 1 & T`); one fixture type with `any` three members deep is passed to it on a line marked `// @ts-expect-error` | Exit status 0: the calls type-check, each marked line is an error, and no argument or result type holds `any` at any depth |
| Every operation, as the parity cases call it and with an undeclared option | The call returns a promise and throws nothing; the undeclared option rejects it with `invalid-argument` |
| `project validate` of the long program of [VER-EVALUATION-TIME-LIMIT](verification-programs.mara.md#01M7HZYKBNDY3ZA77JMB0VD06H) with a time limit of 2 s, embedded and through the daemon, a 10 ms interval timer started just before the call | The promise settles after at least 2 s; no gap exceeds 200 ms between the call and the first callback, between two callbacks, or between the last callback before settling and the settling |
| The embedded client in direct mode, the package's `mara` binary removed and none on `PATH` | The Node parity cases pass, so no operation runs the CLI |
| The client through the daemon, after a daemon of the test was started and its session opened, with the package's `mara` binary made non-executable (`chmod a-x`, which keeps its path, size and modification time): the Node parity operations | Each is served by that daemon ([VER-DAEMON-START](verification-daemon.mara.md#01M7JKW7JWB2RANYKNE6BHYM26)) with the result of CLI direct mode, so no operation starts a CLI process |
| The embedded client in direct mode: a read, a document edited, the same read | The second read returns the result of CLI direct mode on the edited files |

---

<a id="01M7JQJKGBSCMPWE7CGNPP7PNB"></a>

### Work on supplied files in a browser

| Type | Verification |
| --- | --- |
| ID | VER-BROWSER-WORKSPACE |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-BROWSER-LOCAL-WORKSPACE](hosts.mara.md#01M7BF6H0G63VTGGYK0HSE5XDE) |

In the browser setup of [VER-HOST-COMPARISON](verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9), which holds the parity cases, a page of the test creates a workspace from the files of a disposable project of `empty` with a schema, items in two documents and no program, `.git` left out ([DES-BROWSER-WORKSPACE](hosts.mara.md#01M7JQJKGBJ7KQFM9PJV8RK8H7)). Before the package loads, the page wraps `WebAssembly.compile`, `compileStreaming`, `instantiate`, `instantiateStreaming`, `Module` and `Instance` to count their calls. A copy of the project receives the same writes in CLI direct mode; its files are the expected bytes, MIDs paired as the parity cases pair them. The test computes the BLAKE3 hash of each supplied file.

| Case | Expectation |
| --- | --- |
| The workspace created, then `item list` and `get` of an item | Playwright reports a dedicated worker that the package started; the page's wrappers counted no call; both results are those of CLI direct mode |
| `item create` in an existing document and in a new one, `item update`, `item move` from one document to the other, `relation add`; then an export | One entry per file the copy's writes changed and no other: its bytes are the copy's; a supplied file's `base` is the hash of its supplied bytes; the new document has no `base` |
| A document changed by two writes | One entry: the bytes after the second write, and the hash of the supplied bytes, not of the bytes after the first |
| The caller removes a supplied document; writes another with new bytes, then with its supplied bytes; adds a file, then removes it | A removal entry with the removed file's `base`, and no entry for the other two |
| The caller writes a document with an added item, then `get` of that item | The item as the written bytes hold it |
| The caller writes, each followed by `project validate` and `item list`: the configuration with an include pattern that leaves one document out; the schema without a type that items use; a `.gitignore` that ignores a document; from mara-xsm8w6, a program with an invariant that has rows and the configuration that declares it | The results of CLI direct mode on the same changed files |
| `item create` at a line inside an item | Fails with `invalid-position`, as in CLI direct mode; the next export equals the one before |
| Two exports, then `item list` | Equal change sets; the list holds the created items |
| After the cases above | The supplied buffers hold their bytes; `localStorage`, `sessionStorage`, IndexedDB, Cache Storage and the origin-private file system of the page's origin hold nothing; every request that Playwright reports for the page and its workers, and every one the test's server receives, is for a file of the package; no daemon of the test runs |

---

<a id="01M7JQJKGBJ6VC7X844R6HZZGS"></a>

### Report what a host cannot do as unavailable

| Type | Verification |
| --- | --- |
| ID | VER-UNAVAILABLE-NOT-PARTIAL |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-UNAVAILABLE-NOT-PARTIAL](engine.mara.md#01M7BF6GXSJVVASJ4C8ZGHM909) |

In the setup of [VER-BROWSER-WORKSPACE](#01M7JQJKGBSCMPWE7CGNPP7PNB). On the native hosts, code that cannot be read is checked by [VER-CODE-FACTS](verification-code-use.mara.md#01M7JKT47QVCRS95RJC08VA021) and [VER-TEMPLATE-CASES](verification.mara.md#01M7CT60NQK9TAH7JD15HCR2XE), and a grammar that cannot be loaded by [VER-GRAMMAR-MARKERS](verification-code.mara.md#01M7JKT47QR2NYFK59JRV8EV7G).

| Case | Expectation |
| --- | --- |
| `project init` of each template, and `project convert` of the 0.3.0 input of [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA), in a workspace and through the daemon from Chromium | Fail with `unavailable-operation` naming the operation; the next export is empty, and the daemon's project is byte-identical |
| The fixtures of [VER-GRAMMAR-MARKERS](verification-code.mara.md#01M7JKT47QR2NYFK59JRV8EV7G), with both grammars pinned, in a workspace: each operation that reads code | The result of CLI direct mode on the same files with both grammars removed from the cache, but for each `unavailable-grammar` message, which says the browser cannot load grammars; `project validate` is invalid |
| The setup of [VER-PROGRAM-FAULTS](verification-programs.mara.md#01M7HZYKB4R632Q63YWS8CY1FJ) with a program that passes, in a workspace whose Soufflé module the test's server answers with status 404, and in a second run with a truncated module | A fault without a line at the configuration line of each declared program; `project validate` is invalid, `query execute` and `query view` fail, and the gate and both items are unavailable |
