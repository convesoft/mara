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
| Node library | Embedded or through the daemon | Filesystem |
| Browser through daemon | Loopback HTTP ([REQ-BROWSER-ACCESS-AUTHORIZED](daemon.mara.md#01M7BF6GR0N9J4C8Z5QQ4CSCB7)) | Daemon's filesystem |
| Browser, local | Engine as WASM in a worker | Supplied file contents |

---

<a id="01M7BF6H124JRJAQSYVQ21Q2JS"></a>

### Operations

| Type | Design |
| --- | --- |
| ID | DES-OPERATIONS |
| Status | Draft |
| Satisfies | [REQ-CLI-MCP-PARITY](#01M7BF6H00N50GBQGCYF4RSR1Z) |

Names follow the 0.3.0 CLI where the capability is kept.

| Group | Operations |
| --- | --- |
| `project` | `init`, `validate`, `fix`, one-shot conversion |
| `schema` | `get`, `list`, `validate` |
| `item` | `create`, `update`, `move`, `rename`, `delete`, `list`, `validate` |
| `relation` | `add`, `remove` |
| Read | `search`, `get`, `related` |
| Rules | Query, view, gate, report |
| `code` | `add` |
| `daemon` | `status`, `start`, `stop` |
| `mcp` | Serves the document operations (every group above except `code` and `daemon`) over stdio |
| `prime` | Prints the agent guide ([DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8)) |

- An MCP server takes the project on each call, or is bound to one project when started with `--project`.

---

## Open questions

- How the Node library embeds the engine: the native addon of [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8), or the WASM build.
