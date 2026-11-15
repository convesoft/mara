# Hosts

## Requirements

---

<a id="01M7BF6H00N50GBQGCYF4RSR1Z"></a>

### CLI and MCP expose the same operations

| Type | Requirement |
| --- | --- |
| ID | REQ-CLI-MCP-PARITY |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

Every document operation is available from the CLI and from the MCP server with the same inputs and results; `--format json` prints what MCP returns. An undeclared option or argument is rejected before anything runs.

---

<a id="01M7BF6H09NQCQTEZA4WEZAXEZ"></a>

### Use Mara from Node

| Type | Requirement |
| --- | --- |
| ID | REQ-NODE-LIBRARY |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

A Node application imports Mara as a typed, asynchronous ESM library and runs operations without spawning the CLI, embedded or through the daemon.

---

<a id="01M7BF6H0G63VTGGYK0HSE5XDE"></a>

### Work on a project inside the browser

| Type | Requirement |
| --- | --- |
| ID | REQ-BROWSER-LOCAL-WORKSPACE |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

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
| Satisfies | [REQ-SAME-RESULT-ON-EVERY-HOST](engine.mara.md#01M7BF6GXHZNABX9R7NKPNGJNM) |

| Mode | Host | Input access |
| --- | --- | --- |
| CLI, direct | In-process | Filesystem |
| Daemon | One per OS user and Mara version; serves CLI, MCP and Node over native IPC, browsers over loopback HTTP | Filesystem |
| Node library | Embedded through the native addon ([DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8)), or through the daemon | Filesystem |
| Browser through daemon | Loopback HTTP ([REQ-BROWSER-ACCESS-AUTHORIZED](daemon.mara.md#01M7BF6GR0N9J4C8Z5QQ4CSCB7)) | Daemon's filesystem |
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
| `prime` | Prints the agent guide ([DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |

- An MCP server takes the project on each call, or is bound to one project when started with `--project`.
- The CLI's global options stand anywhere: `--project <dir>`, `--format human|json` (default `human`) and `--debug-stats` ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). An operation's options follow its words, as `--<name> <value>` or `--<name>=<value>`. Bare `mara` prints the usage and exits 0.
- Exit status: 0 for a valid result or a successful operation, 1 for an invalid result or a failed request, 2 for rejected arguments.
- `--format json` prints one JSON object on stdout: the result, or `{"format_version": 1, "error": {"code", "message", "diagnostics"}}` when the request fails, `diagnostics` holding the problems in project files that caused it. Each result also carries `format_version` 1.
- The human form prints a result on stdout, and a failure on stderr as `mara: error[<code>]: <message>` followed by its diagnostics ([DES-DIAGNOSTICS](validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)).
- Besides the selection codes of [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27), a request fails with these codes; a failure's message lists names as a diagnostic's does ([DES-SCHEMA-CHECKS](validation.mara.md#01M7BF6HG7AQZ9FM38CMT4BTHR)):

| Code | Failure |
| --- | --- |
| `invalid-argument` | An option or argument the operation does not declare, a missing, repeated or malformed one (exit status 2) |
| `invalid-project` | The configuration or the schema has an error, so a read refuses the project; `diagnostics` holds their diagnostics |
| `unknown-item` | No item has the ID or MID given |
| `ambiguous-item` | Several items have the ID or MID given |
| `unknown-node` | A path, or path and line, names no document, item or narrative unit that the read takes |
| `unknown-declaration` | The schema declares no type or relation of the name given |
| `invalid-utf8` | `get` or an edit of a document that is not UTF-8 |
| `changed-file` | A file the edit writes changed after the edit was computed, or appeared where it creates one ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) |
| `existing-file` | A file `project init` would write exists, or `.mara` is no directory ([DES-PROJECT-INIT](init.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)); nothing is written |
| `unreadable-edit` | The new contents of a document would not read back as the edit writes them, or would change how text it does not touch reads, such as an item inside an unclosed code block or a link reference definition that other text uses removed ([DES-ITEM-WRITER](authoring.mara.md#01M7BF6GP2Z6R1VCQMQ6ZZB218)); nothing is written |
| `invalid-position` | A line where an edit would put an item lies inside an item or past the end of the document ([DES-MOVE-AND-RENAME](authoring.mara.md#01M7FWNBFDHGXN2TH1BK08GKCK)) |
| `not-an-input` | A document that an edit would create would be no input of the project ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)) |
| `duplicate-id`, `wrong-id-prefix`, `missing-row` | A rename would cause that diagnostic, or the item has no `ID` row to rename ([DES-MOVE-AND-RENAME](authoring.mara.md#01M7FWNBFDHGXN2TH1BK08GKCK)) |
| `stale-cursor` | The cursor belongs to another result |
| `record-too-large` | A record exceeds a page ([DES-PAGING](retrieval.mara.md#01M7BF6HCCWQH6P13ZR2YHAPS0)) |
