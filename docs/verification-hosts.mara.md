# Setup and host verification

---

<a id="01M7F74J51BE6J8BQSH842BXWV"></a>

### Initialize a project from each template

| Type | Verification |
| --- | --- |
| ID | VER-PROJECT-INIT |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-INIT](project.mara.md#01M7BF6H8Z02SNKXD2XPGVD87M) |

Rust integration tests run `project init` with the real CLI in disposable directories.

| Case | Expectation |
| --- | --- |
| No template named, in the working directory and in a named one | `.mara/config.toml` and `.mara/schema.toml` are written there; the output lists exactly the files created; `project validate` reports no diagnostic |
| `schema list` and `schema get` of that project | Every type and relation of [DES-ENGINEERING-VOCABULARY](vocabulary.mara.md#01M7BF6HH8CA2B9TQJ9DT32WDG), with its ID prefix, description, fields, allowed values, target types and constraints, and nothing else |
| Items of that project with each of the statuses `Draft`, `Accepted` and `Retired`, and with another status | Valid; invalid |
| `empty` named | A configuration and a schema that declares no type and no relation; `project validate` reports no diagnostic |
| A directory in which one of the files to write exists | Refused, naming that file; no file or directory is created |
| The programs of `engineering`, from 0.4.0-alpha.3 (mara-tx0fcf) | Written under `.mara/rules/` and listed |

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
| The 0.3.0 project's conversion stopped while it writes the report file and after each document, one of them a document whose conversion leaves something out, then rerun | As [VER-INTERRUPTED-EDITS](verification-writes.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) requires |
| After a stop, a `[[ID]]` mention added by hand to a converted document from which a symmetric relation row was dropped, then a rerun | The returned report and the report file still hold that omission, once |
| After a stop, a rerun with one unconverted document unchanged and another edited so that one of its omissions moves to another line | Each unchanged entry once; for the moved omission, the recorded entry and the rerun's entry at its new line |
| A report file left by an earlier run that is a symbolic link, is not JSON of that object, or has `format_version` 2 | Refused, naming the file; nothing written |
| Every other operation on a 0.3.0 project | Refused, as [VER-NESTED-PROJECTS](verification-documents.mara.md#01M7D3NAR43MYRDF7YKMBDSJ10) requires |
| A `:::mara` block in a converted project's document | Read as narrative: it makes no item |

---

<a id="01M7F74J5Z4KX9F3HN78T1WB85"></a>

### Send each request through the CLI and MCP

| Type | Verification |
| --- | --- |
| ID | VER-CLI-MCP-PARITY |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-CLI-MCP-PARITY](hosts.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |

Rust integration tests send each request through the real CLI with `--format json` and through the real stdio MCP server, `mara mcp`, on one disposable project restored to the same files at the same path between the two. Each MID that a request generates, as `item create` and `project fix` do, differs between the two runs. The test pairs those MIDs by the path and line of their anchors, which are the same in both runs, and replaces the MCP run's with the CLI run's in its result and files. A `revision` of an item that holds a generated MID must be the hash of that item's region in its own run, and is left out of the comparison.

| Case | Expectation |
| --- | --- |
| The server's tool list | One tool per document operation of [DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS), named as it says, and no other; each tool's parameters are the arguments and options of its CLI operation |
| Each document operation with the same inputs, failing requests included | After the replacement, the tool returns the JSON object the CLI prints, and the project's files are the same after both runs |
| An undeclared option or argument, on each surface | Rejected with `invalid-argument`, exit status 2 on the CLI, before anything runs: the CLI's debug stats count no work and no file changes |
| A server started with `--project`, and one given the project on each call | Every call runs against that project |
| The `query` operations, from 0.4.0-alpha.3 (mara-bx82k0) | As every other document operation |

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
| Inside a project, found from the working directory or named by `--project` | Both open with the same fixed heading and have the sections find and read, author, repair after hand edits, and validate; operations appear as commands in `prime` and as tools in the MCP instructions |
| Outside every project | Both say how to initialize a project and name no other operation or tool |
| Each operation, option, tool and parameter that either rendering names | The CLI declares it or the MCP server lists it, compared as [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) describes |
| Two projects whose schemas differ | The same guide |
| A project whose configuration has an error | The full guide |
| The MCP instructions | At most 3,800 characters |
