# Agent guidance

## Requirements

---

<a id="01M7BGRHWB2MGXEY5G41YP8E0G"></a>

### Mara teaches agents its own workflow

| Type | Requirement |
| --- | --- |
| ID | REQ-AGENT-GUIDE |
| Status | Accepted |
| Satisfies | [NEED-AGENT-ONBOARDING](product.mara.md#01M7BGRHXED9E91E4E7P0T5WT5) |

`mara prime` and the MCP server's instructions deliver the same agent workflow guide when a project is found. Without a project, both deliver only a note on how to initialize one. The guide names only operations, options and tools that the binary has.

---

## Design

---

<a id="01M7BGRHWQHQJJHC6XCXVDWDM8"></a>

### Agent guide

| Type | Design |
| --- | --- |
| ID | DES-AGENT-GUIDE |
| Status | Accepted |
| Satisfies | [REQ-AGENT-GUIDE](#01M7BGRHWB2MGXEY5G41YP8E0G) |

- One template compiled into the binary is rendered per surface: operations appear as commands in `mara prime` and as tools in the MCP instructions, options as CLI options and as tool parameters of the same name ([DES-MCP-SERVER](hosts.mara.md#01M7J0N6KRQ3NNE4Z6SPZ1VYRK)).
- The guide depends only on the binary. It lists none of the project's types or relations and sends the agent to schema inspection first ([REQ-SCHEMA-INSPECTION](project.mara.md#01M7BF6H9F8E7RB3XBSE0EKAR3)).
- Sections: find and read, author, repair after hand edits, validate. The repair section shows an item written by hand without an anchor and with bare relation targets, and the `---` before it, which every item needs unless it starts the document; `project fix` then completes it.
- Operation, option and tool names in the guide come from the definitions that the CLI and the MCP tool schemas use. Tests compare them in both directions: the guide names nothing undeclared, and names every operation that works in a project. `project init` and `project convert` appear only in the notes below; `prime`, `mcp` and `code add`, which are no tools, in neither.
- The project is the one found where the surface starts ([REQ-ONE-PROJECT-PER-OPERATION](project.mara.md#01M7BF6H8ND7YSXFMWCM0KXAAY)): the working directory or `--project` for `prime`, the server's start for MCP.

| Surface | Project found | No project |
| --- | --- | --- |
| `mara prime` | Full guide | How to initialize a project, and nothing more |
| MCP server instructions | Full guide | How to initialize a project, and nothing more |

- In a 0.3.0 project, both renderings say only how to convert it with `project convert`. `--project` naming a directory that is no project root gets the note on initialization. Any other selection error, such as a `.mara` that is a symbolic link, fails as it does for every operation.
- `mara prime --format json` prints `{"format_version": 1, "text"}`, `text` holding what the human form prints.
- The MCP rendering fits the server: a server started without `--project` says that each tool takes `project` and to pass it for another checkout or worktree, and its init note names `project` as the directory to initialize; one started with `--project` says that its tools take none. Otherwise both are the same. A server whose selection fails otherwise sends no instructions; each call reports the error.
- The full guide opens with the same fixed heading in both renderings, so a project's agent instructions can send an agent to `mara prime` only when its context lacks the guide.
- The MCP rendering stays within 3,800 characters; Claude Code truncates server instructions at 4,096. It leaves out what the tool schemas carry.
- Project conventions stay in the project's own agent instructions.
