# Mara

Mara keeps project knowledge (goals, requirements, designs, decisions and the links between them) as typed, validated items inside ordinary Markdown. One file is both the editable source and the documentation GitHub renders. A CLI, an MCP server, a Node library and a browser build read, edit, validate and query it.

## Status

v0.4.0 is a rewrite from scratch and cannot be used yet: this repository holds its specification, its task list and the code written so far.

The last release is 0.3.0 (`npx -y @convesoft/mara@0.3.0`, source at tag `v0.3.0`). It uses the `:::mara` syntax that v0.4.0 replaces; see [migration](docs/migration.mara.md).

## Start

| To | Go to |
| --- | --- |
| See what an item looks like | [Item format](docs/format.mara.md) |
| Read the specification | [docs/index.mara.md](docs/index.mara.md) |
| Find work | `zban list --ready`, or the Markdown issues in `.zban/`: one epic per release slice, `0.4.0-alpha.1` to `0.4.0` |
| Contribute, as a person or an agent | [AGENTS.md](AGENTS.md) |

## Licence

MIT or Apache-2.0, at your option.
