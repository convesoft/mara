# Hosts

| Page | Holds |
| --- | --- |
| [Host verification](verification.mara.md) | Checks of CLI and MCP parity, the Node library and the browser workspace |

## Requirements

---

<a id="01M7BF6H00N50GBQGCYF4RSR1Z"></a>

### CLI and MCP expose the same operations

| Type | Requirement |
| --- | --- |
| ID | REQ-CLI-MCP-PARITY |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](../product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

Every document operation is available from the CLI and from the MCP server with the same inputs and results; `--format json` prints what MCP returns. An undeclared option or argument is rejected before anything runs.

---

<a id="01M7BF6H09NQCQTEZA4WEZAXEZ"></a>

### Use Mara from Node

| Type | Requirement |
| --- | --- |
| ID | REQ-NODE-LIBRARY |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](../product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

A Node application imports Mara as a typed, asynchronous ESM library and runs operations without spawning the CLI, embedded or through the daemon.

---

<a id="01M7BF6H0G63VTGGYK0HSE5XDE"></a>

### Work on a project inside the browser

| Type | Requirement |
| --- | --- |
| ID | REQ-BROWSER-LOCAL-WORKSPACE |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](../product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

In a browser the engine runs as WASM in a worker over supplied file contents. Edits change only that in-memory workspace and are exported as a change set: the final contents of added, changed and deleted files, with the hashes of the base contents they apply to.

---

## Design

---

<a id="01M7BF6H0SD0MTC8WCT6GDY9AN"></a>

### Execution modes

| Type | Design |
| --- | --- |
| ID | DES-EXECUTION-MODES |
| Status | Accepted |
| Satisfies | [REQ-SAME-RESULT-ON-EVERY-HOST](../engine.mara.md#01M7BF6GXHZNABX9R7NKPNGJNM) |

| Mode | Host | Input access |
| --- | --- | --- |
| CLI, direct | In-process | Filesystem |
| Daemon | One per OS user and Mara version; serves CLI, MCP and Node over native IPC, browsers over loopback HTTP | Filesystem |
| Node library | Embedded through the native addon ([DES-NODE-LIBRARY](#01M89K1J6C11NRR3S3X1KXQ5AF)), or through the daemon | Filesystem |
| Browser through daemon | Loopback HTTP ([REQ-BROWSER-ACCESS-AUTHORIZED](../daemon/index.mara.md#01M7BF6GR0N9J4C8Z5QQ4CSCB7)) | Daemon's filesystem |
| Browser, local | Engine as WASM in a worker | Supplied file contents |

---

<a id="01M7BF6H124JRJAQSYVQ21Q2JS"></a>

### Operations

| Type | Design |
| --- | --- |
| ID | DES-OPERATIONS |
| Status | Accepted |
| Satisfies | [REQ-CLI-MCP-PARITY](#01M7BF6H00N50GBQGCYF4RSR1Z) |

Every operation is `<group> <verb>`, except `search` and `get`. An MCP tool is named `<group>_<verb>`.

| Group | Operations |
| --- | --- |
| `project` | `init`, `validate`, `fix`, `convert` |
| `schema` | `get`, `list`, `validate` |
| `item` | `create`, `update`, `move`, `rename`, `delete`, `list`, `validate` |
| `relation` | `add`, `remove`, `list` |
| Top level | `search`, `get` |
| `query` | `execute`, `view`, `gate`, `report`, `list` |
| `code` | `add` |
| `daemon` | `status`, `start`, `stop` |
| `mcp` | Serves the document operations (every group above except `code` and `daemon`) over stdio |
| `prime` | Prints the agent guide ([DES-AGENT-GUIDE](../agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |

- An MCP server takes the project on each call, or is bound to one project when started with `--project` ([DES-MCP-SERVER](#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)).
- The CLI's global options stand anywhere: `--project <dir>`, `--format human|json` (default `human`), `--debug-stats` ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)) and `--no-daemon`, the direct-mode flag ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)). An operation's options follow its words, as `--<name> <value>` or `--<name>=<value>`; the argument after `--<name>` is its value whatever it reads as, so `--id --no-daemon` names an ID and leaves direct mode off. `--` ends no options: it is an unknown option. Bare `mara` prints the usage and exits 0. `mara --version` prints `mara <version>`, the workspace version, on stdout and exits 0. The top-level `mara --version` invocation takes no other argument. Until the parser has read a complete operation name, it treats `--version` in option position as the top-level flag. The flag is outside the document operation set.
- An option that does not repeat, given twice, is rejected with `invalid-argument`; its last value never wins. The CLI rejects it before it sends a request, in direct mode and with a daemon, and MCP takes one value for such a parameter, so an array is mistyped ([DES-MCP-SERVER](#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)). The daemon checks a request again by the same rule ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)) and refuses one that repeats such an option with `malformed-request`, without running it; no client of its build sends one.
- Exit status: 0 for a valid result or a successful operation, 1 for an invalid result or a failed request, 2 for rejected arguments.
- `--format json` prints one JSON object on stdout: the result, or `{"format_version": 1, "error": {"code", "message", "diagnostics"}}` when the request fails, `diagnostics` holding the problems in project files that caused it. Each result also carries `format_version` 1.
- The human form prints a result on stdout, and a failure on stderr as `mara: error[<code>]: <message>` followed by its diagnostics ([DES-DIAGNOSTICS](../validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)).
- Besides the selection codes of [DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) and the write codes of [DES-AUTHORING-REQUESTS](../authoring/index.mara.md#01M7FWQ5YB6X95VCRVSP1FGQ46), a request fails with these codes; a failure's message lists names as a diagnostic's does ([DES-SCHEMA-CHECKS](../validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)):

| Code | Failure |
| --- | --- |
| `invalid-argument` | An option or argument the operation does not declare, a missing, repeated or malformed one (exit status 2) |
| `invalid-project` | The configuration or the schema has an error, so a read refuses the project; `diagnostics` holds their diagnostics |
| `unknown-item` | No item has the ID or MID given |
| `ambiguous-item` | Several items have the ID or MID given |
| `unknown-node` | A path, or path and line, names no document, item or narrative unit that the read takes |
| `unknown-declaration` | The schema declares no type or relation of the name given |
| `invalid-utf8` | `get` or an edit of a document that is not UTF-8 |
| `changed-file` | A file the edit writes changed after the edit was computed, or appeared where it creates one ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) |
| `existing-file` | A file `project init` would write exists, or `.mara` is no directory ([DES-PROJECT-INIT](../project.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)); nothing is written |
| `unreadable-edit` | The new contents of a document would not read back as the edit writes them, or would change how text it does not touch reads, such as an item inside an unclosed code block or a link reference definition that other text uses removed ([DES-ITEM-WRITER](../authoring/index.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)); nothing is written |
| `invalid-position` | A line where an edit would put an item lies inside an item or past the end of the document ([DES-MOVE-AND-RENAME](../authoring/index.mara.md#01M7FWNBFDHGXN2TH1BK08GKCK)) |
| `not-an-input` | A document that an edit would create would be no input of the project ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)) |
| `duplicate-id`, `wrong-id-prefix`, `missing-row` | A rename would cause that diagnostic, or the item has no `ID` row to rename ([DES-MOVE-AND-RENAME](../authoring/index.mara.md#01M7FWNBFDHGXN2TH1BK08GKCK)) |
| `not-a-legacy-project` | `project convert` of a project that is not a 0.3.0 project ([DES-CONVERSION-MAPPING](../migration.mara.md#01M7CMW71TXDY6VCTJS7XNSQRM)) |
| `daemon-lost` | The daemon ended before it answered a request sent to it ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)); a write may be left interrupted ([REQ-INTERRUPTED-EDIT-LOSES-NOTHING](../authoring/writes.mara.md#01M7BF6HJ3758Y06V61901ZSKK)) |
| `unavailable-operation` | The host cannot run the operation, such as `project init` in a browser ([DES-BROWSER-WORKSPACE](#01M7JQJKGBJ7KQFM9PJV8RK8H7)); nothing is written |
| `daemon-running` | `mara daemon start` asks for another browser listener than the running daemon of the build has ([DES-BROWSER-LISTENER](../daemon/index.mara.md#01M7CQ57TJY788DMPBN4BX1SEM)); nothing changes |
| `stale-cursor` | The cursor belongs to another result |
| `unknown-entry` | No entry of the operation's kind has the name given ([DES-QUERY-RESULTS](../rules/index.mara.md#01M7JT87HXDH2D4V1FNM6VZT7N)) |
| `missing-input`, `unlisted-input`, `repeated-input`, `invalid-input` | The request's inputs do not fit its entry's ([DES-REQUEST-INPUTS](../rules/index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)) |
| `program-failed` | The program of a query or view gives no rows: refused, not fitting its entry, not compiling or faulting; `diagnostics` holds why |
| `record-too-large` | A record exceeds a page ([DES-PAGING](../retrieval/index.mara.md#01M7BF6HCCWQH6P13ZR2YHAPS0)) |

---

<a id="01M7J0N6KRQ3NNE4Z6SPZ1VYRK"></a>

### MCP server

| Type | Design |
| --- | --- |
| ID | DES-MCP-SERVER |
| Status | Accepted |
| Satisfies | [REQ-CLI-MCP-PARITY](#01M7BF6H00N50GBQGCYF4RSR1Z) |

- `mara mcp` speaks MCP over stdio through `rmcp` ([ADR-RMCP-SERVER](#01M7J0N6M9GKB113E35X5QPTZY)): the 2026-07-28 revision, whose requests carry their version, and the `initialize` handshake of 2024-11-05 to 2025-11-25.
- `tools/list` returns one tool per document operation of [DES-OPERATIONS](#01M7BF6H124JRJAQSYVQ21Q2JS) on one page. Its description is the operation's summary.
- A positional argument is a required string parameter. An option is a string parameter, an integer for a whole number (`limit`, `line`), an array of them when it repeats; `title` and `body` of `item create` and `id` of `query gate` and `query report` are required. `input` of the `query` tools is an object ([DES-REQUEST-INPUTS](../rules/index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)). A parameter set to `null` counts as left out.
- A server started without `--project` adds a `project` parameter to every tool: the project root, absolute or relative to the server's working directory. Without it, the project is found from the server's working directory upward, as the CLI finds it from its own. A server started with `--project` lists no `project` parameter, and a call that gives one is rejected.
- A call's result is one text content block holding the JSON object that `--format json` prints, with `isError` true for a failure; there is no structured content. An undeclared, missing or mistyped parameter is rejected with `invalid-argument` before the operation runs, in a message that names the parameter. An unknown tool is a JSON-RPC error.
- Calls run on the runtime's blocking pool, several at once if the client sends them so. Each is a request of a client of the daemon, from the server's working directory ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)): the daemon of the build serves it, or the server serves it in-process when none runs and starts one. The server keeps nothing between calls. Writes wait for the write lock ([REQ-WRITERS-SERIALIZED](../authoring/writes.mara.md#01M7CXC2N8EZYVHGY7T2GPPB3X)). A call that the client cancels still runs to its end; only its response is dropped.
- Its `instructions` are the agent guide ([DES-AGENT-GUIDE](../agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)).
- With `--debug-stats`, the server prints one stats line when it exits: the work of all its calls.

---

<a id="01M7JQJKGBJ7KQFM9PJV8RK8H7"></a>

### Browser workspace

| Type | Design |
| --- | --- |
| ID | DES-BROWSER-WORKSPACE |
| Status | Accepted |
| Satisfies | [REQ-BROWSER-LOCAL-WORKSPACE](#01M7BF6H0G63VTGGYK0HSE5XDE) |
| Satisfies | [REQ-UNAVAILABLE-NOT-PARTIAL](../engine.mara.md#01M7BF6GXSJVVASJ4C8ZGHM909) |

- A workspace starts from supplied files, each a path relative to the project root with its bytes. The caller may later write a file's bytes or remove a file; such a change is an edit, as a Mara write is.
- It runs every document operation but `project init` and `project convert`, which fail with `unavailable-operation` and change nothing. A browser that reaches the daemon gets the same failure for them ([DES-BROWSER-LISTENER](../daemon/index.mara.md#01M7CQ57TJY788DMPBN4BX1SEM)).
- It loads no grammar: every pinned grammar is `unavailable-grammar`, with a message that the browser cannot load grammars, and its language's files are scanned at the basic level ([DES-GRAMMAR-ASSETS](../code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)).
- A Soufflé module that cannot be loaded or compiled is a fault without a line for each evaluation ([DES-RULE-EXECUTION](../rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).
- Export returns the change set and leaves the workspace as it is. The change set has one entry per path whose final contents differ from its supplied contents: the final bytes, or none for a removed file, and `base`, the BLAKE3 hash of the supplied bytes in the form of `revision` ([DES-READ-RESULTS](../retrieval/index.mara.md#01M7DMT8GAJCAFTW0Y2939JFF1)), or none for a file that was not supplied. A file written back to its supplied bytes, or added and removed again, has no entry.
- A supplied path is relative to the project root, in the form a diagnostic gives it: `/` between parts, none of them empty, `.` or `..`, and no NUL. A path in another form, supplied twice or below another supplied file, a write to a directory or below a file, and a removal where no file is, are refused with `invalid-argument`; so are an unknown operation, arguments that are no object or that cannot be sent to the worker, and a `project` argument, since the workspace holds one project. `project init` and `project convert` fail before their arguments are read.
- A request runs as its MCP tool's call does ([DES-MCP-SERVER](#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)), against inputs that the workspace keeps between requests as a daemon's session keeps them ([DES-DAEMON-SESSIONS](../daemon/index.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7)). The host library, the `mara` crate with the engine, is compiled to WebAssembly for it, not the engine alone. A read of contents from its memory counts no file read, as [VER-WORK-COUNTERS](../verification.mara.md#01M7BF6HGG6ET88HS641Y5676G) states for the contents that the caller supplies or writes.
- The files lie in the module's memory ([ADR-MEMORY-FILESYSTEM](#01M896GJ66CDBJE28PYCWXYBC0)), which gives each change of a file's contents or of a directory's entries a new modification time from a logical clock starting at the Unix epoch: a stat shows every change, so a check settles what it read at once, whatever the worker's clock reads ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)). As a Git checkout, the memory holds no empty directory: a directory goes with its last file.
- The browser package's JavaScript API, an ES module with its TypeScript declarations; every method but `close` returns a promise and throws nothing:

| Call | Does |
| --- | --- |
| `createWorkspace(files)` | Starts a workspace in a dedicated worker of its own, which loads the package's WebAssembly module ([ADR-RAW-WASM-MODULE](#01M896GJAKXNE1E6VXT4JYD2WQ)). `files` is an iterable of `{path, contents}`, the contents a `Uint8Array` or an `ArrayBuffer`, which the call copies and leaves as they are |
| `workspace.run(name, args, options)` | Runs the document operation whose MCP tool is `name` with the tool's arguments `args`, and resolves with the object that `--format json` prints. A failure rejects with a `MaraError` holding `code`, `message`, `diagnostics` and `reply`, the printed object. With `options.debugStats` true it resolves with `{result, stats}`, and a `MaraError` holds `stats` |
| `workspace.write(path, contents)`, `workspace.remove(path)` | The caller's write and removal |
| `workspace.export()` | Resolves with the change set in path order, each change `{path, contents, base}`: `contents` a `Uint8Array`, or `null` for a removed file, and `base` a string, or `null` for a file that was not supplied |
| `workspace.close()` | Ends the worker; every later call rejects |

---

<a id="01M89K1J6C11NRR3S3X1KXQ5AF"></a>

### Node library and typed clients

| Type | Design |
| --- | --- |
| ID | DES-NODE-LIBRARY |
| Status | Accepted |
| Satisfies | [REQ-NODE-LIBRARY](#01M7BF6H09NQCQTEZA4WEZAXEZ) |

- `@convesoft/mara` is the Node library, an ES module with its TypeScript declarations, provisional until mara-2c6hfk: `lib/index.js`, `lib/index.d.ts` and `lib/operations.d.ts`, exported as the package's `.`, with `"type": "module"`. mara-865a2e packs them and the addon.
- The addon is `bin/mara.node` of the native package that the dispatcher runs `bin/mara` of: the package for `process.platform` and `process.arch`, installed for the dispatcher at its version ([DES-PACKAGES](../distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8)). It loads once per process, on the first call; a call that cannot load it rejects with an `Error` holding the dispatcher's message.
- The addon is the `mara` library built as an N-API library ([ADR-NAPI-RS-ADDON](#01M89K1EVRVANHY87X8RPYHT5B)). It exports one function, `call(name, arguments, project, noDaemon)`, which runs on a thread of libuv's pool, so the event loop never waits for an operation. A call runs as a tool call of `mara mcp` does ([DES-MCP-SERVER](#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)): through the daemon of the addon's build, or in-process, which then starts one, or in-process only in direct mode: `noDaemon`, `CI` or `MARA_NO_DAEMON=1`.
- The addon is a host embedded in Node: its build is the file `mara` in the addon's own folder, as the dynamic loader names it, which it reaches as its daemon, starts with `daemon start` and finds Soufflé beside ([DES-DAEMON-LIFECYCLE](../daemon/index.mara.md#01M7BF6GR80WA207P4BBFWW671)). It leaves the process's signals to Node ([DES-RULE-EXECUTION](../rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)). In direct mode it needs no `mara` file.
- `lib/operations.d.ts` types every client: each document operation by its tool name, its arguments as its MCP tool takes them, a required one required, and its result as `--format json` prints it, no type holding `any`. `mara-browser:build` copies it into the browser package, whose workspace and daemon client implement its `Client`.
- A client holds one project, so a `project` argument is refused with `invalid-argument`, as in a browser workspace.
- `run`, the one method of each client, takes the tool's name and arguments and resolves with the result, or with `{result, stats}` when `options.debugStats` is true; a failure rejects with a `MaraError` holding `code`, `message`, `diagnostics`, `reply` and, with debug stats, `stats` ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). It throws nothing.

| Call | Package | Does |
| --- | --- | --- |
| `createClient({project, noDaemon})` | `@convesoft/mara` | A client of `project`, resolved from the working directory when the client is created, or of the project found from the working directory of each call upward; `noDaemon` is the direct-mode flag |
| `connectDaemon({url, token, project})` | The browser package | A client of `project`, an absolute path that the daemon allows, through its listener at `url`, `http://127.0.0.1:<port>`, with the token that `daemon start --http` printed ([DES-BROWSER-PROTOCOL](../daemon/index.mara.md#01M893WC9SH24JB13G0RNC0XVR)) |
| `createWorkspace(files)` | The browser package | A browser workspace ([DES-BROWSER-WORKSPACE](#01M7JQJKGBJ7KQFM9PJV8RK8H7)) |

- The daemon client sends a tool's positional arguments, options and inputs in the request of the listener, by a table of each tool's parameters that a test compares with the tool list of `mara mcp`. As every client, it checks the arguments before it sends them ([DES-OPERATIONS](#01M7BF6H124JRJAQSYVQ21Q2JS)): their names, presence and types, as `mara mcp` does, with its messages. The listener checks their values; its `malformed-request` refusal, which can then only concern a value, rejects as `invalid-argument` with the listener's message, which is `mara mcp`'s. Another refusal rejects with its code and message, and a request that the browser refuses, such as one without the listener's CORS headers, with `fetch`'s error.

---

## Decisions

---

<a id="01M7J0N6M9GKB113E35X5QPTZY"></a>

### Serve MCP through `rmcp`

| Type | Decision |
| --- | --- |
| ID | ADR-RMCP-SERVER |
| Status | Accepted |
| Justifies | [DES-MCP-SERVER](#01M7J0N6KRQ3NNE4Z6SPZ1VYRK) |

- Chosen by the maintainer on 2026-11-15: the `rmcp` SDK, as in 0.3.0, so that Mara does not maintain the protocol's edges itself. Its async runtime starts only in `mara mcp`, single-threaded.
- Rejected: a hand-written JSON-RPC server on `serde_json`. Every lifecycle, version and error rule of each protocol revision would be Mara's to keep.

---

<a id="01M7CQ57SDRGXWJPC1CAK82Z98"></a>

### Name every operation `<group> <verb>`

| Type | Decision |
| --- | --- |
| ID | ADR-GROUP-VERB-COMMANDS |
| Status | Accepted |
| Justifies | [DES-OPERATIONS](#01M7BF6H124JRJAQSYVQ21Q2JS) |

- Chosen: `<group> <verb>` for every operation. Only `search` and `get` stay top-level, because they return items, narrative and code alike.
- Rejected: a top-level `related`, as in 0.3.0. It lists relations from outside the `relation` group.
- Rejected: `relation get`, as in 0.3.0. A relation is one row, which `relation list` already returns with its location.

---

<a id="01M896GJ66CDBJE28PYCWXYBC0"></a>

### Run the host's own code on files in memory in the browser

| Type | Decision |
| --- | --- |
| ID | ADR-MEMORY-FILESYSTEM |
| Status | Accepted |
| Justifies | [DES-BROWSER-WORKSPACE](#01M7JQJKGBJ7KQFM9PJV8RK8H7) |

- Chosen during the rewrite on 2026-11-25 (mara-jptfbq): the browser workspace runs the `mara` library's operations, inventory and write procedure, built for `wasm32-unknown-unknown`. The part of `std::fs` that they use is replaced there by a filesystem in the module's memory (`mara::system`), which holds the workspace's files, and the worker supplies the time and the random bytes. A result is the CLI's by construction; that build leaves out only the daemon, MCP, grammars, downloads and Soufflé's process.
- The module's instance holds one filesystem, so a worker holds one workspace for its life, as a process holds one filesystem: a second creation in one instance is refused with `invalid-argument`.
- In memory, the worker serializes the requests, which stands for the write lock of DES-WRITE-PROCEDURE, and nothing is durable, so its syncs do nothing.
- Rejected: a second host over the engine, with an inventory, operations and JSON of its own. It would repeat each operation and drift from the CLI's results.
- Rejected: a filesystem handle passed through every host function. It changes every native call site for one host.
- Rejected: `wasm32-wasip1` with a filesystem written in JavaScript. It adds a WASI runtime and a second implementation of files outside the module, and a target to the toolchain.

---

<a id="01M896GJAKXNE1E6VXT4JYD2WQ"></a>

### Export plain functions from the browser's WebAssembly module

| Type | Decision |
| --- | --- |
| ID | ADR-RAW-WASM-MODULE |
| Status | Accepted |
| Justifies | [DES-BROWSER-WORKSPACE](#01M7JQJKGBJ7KQFM9PJV8RK8H7) |

- Chosen during the rewrite on 2026-11-25 (mara-jptfbq): the crate `mara-wasm`, a `cdylib` that Cargo builds for `wasm32-unknown-unknown`, exports `mara_alloc`, `mara_call`, `mara_answer` and `mara_answer_length` and imports `now` and `random` from the module `mara`. A call and its answer are frames in the module's memory: a JSON header's length as a 32-bit little-endian number, the header, then the bytes of the files that the header names by `at` and `length`. The package's worker writes and reads them itself. No tool but the `wasm32-unknown-unknown` target that `rust-toolchain.toml` pins builds the module.
- Rejected: `wasm-bindgen` through nx-rust's `wasm-build` target. Its command-line tool must match the crate's version and be installed in every worktree and on CI, and it generates glue that the package would carry, for an interface of four functions.
- Rejected: `wasm-pack`, whose organisation was sunset in 2025, and Emscripten, a second toolchain for the Rust code.

---

<a id="01M89K1EVRVANHY87X8RPYHT5B"></a>

### Build the Node addon with napi-rs and Cargo

| Type | Decision |
| --- | --- |
| ID | ADR-NAPI-RS-ADDON |
| Status | Accepted |
| Justifies | [DES-NODE-LIBRARY](#01M89K1J6C11NRR3S3X1KXQ5AF) |

- Chosen during the rewrite on 2026-11-25 (mara-vfkgbs): the crate `mara-node`, a `cdylib` over the `mara` library, with `napi` and `napi-derive` of napi-rs, built by nx-rust's plain `build` (`artifacts = []`), whose outputs are the library files; the tests copy `libmara_node.so` into the package they install as `bin/mara.node`. The addon exports one function; the library's JavaScript and declarations are written by hand.
- Rejected: nx-rust's `napi-build` target. It needs `@napi-rs/cli` and generates a loader and declarations for `#[napi]` items, which the library does not use: its types are those of the operations' JSON, not of the addon's one function.
- Rejected: N-API's C functions declared by hand. Mara would keep the binding of every N-API call it makes, which napi-rs keeps.
- Rejected: the JavaScript passing the executable's path to the addon. The addon finds `mara` beside its own file, as `mara` finds Soufflé beside itself, so a loaded addon has one build.
