# Mara

Mara keeps project knowledge (goals, requirements, designs, decisions and the links between them) as typed, validated items inside ordinary Markdown. One file is both the editable source and the documentation GitHub renders. A CLI, an MCP server, a Node library and a browser build read, edit, validate and query it.

## Install the prerelease

v0.4.0 is a rewrite from scratch. When published, `0.4.0-rc.1` is available under the npm dist-tag `next`: the CLI, the MCP server, the daemon and rule evaluation, for Linux x64 and arm64 with glibc 2.35 or later, Landlock for rule evaluation, and Node 24 or later. [Packages and platforms](docs/distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) lists what each later version adds.

`latest` is still 0.3.0 (source at tag `v0.3.0`), which reads the `:::mara` format that v0.4.0 replaces and converts ([migration](docs/migration.mara.md)). For one-off runs, name the version or tag:

```sh
npx -y @convesoft/mara@next --version                       # mara 0.4.0-rc.1
npm install --save-dev --save-exact @convesoft/mara@next    # in a project; then npx mara …
```

## First steps

`mara` below is either launcher above; a `mara` on `PATH` may still be 0.3.0.

| To                                  | Run or read                                                                                         |
| ----------------------------------- | --------------------------------------------------------------------------------------------------- |
| Start a project                     | `mara project init`, the [engineering template](docs/template/vocabulary.mara.md) unless `--template empty` |
| Convert a 0.3.0 project             | [Migration steps](docs/migration.mara.md#steps)                                                     |
| See the types and relations         | `mara schema list`                                                                                  |
| Add an item                         | `mara item create goal docs/first.mara.md --title 'Review example' --body 'Record the review result.' --field status=draft`, or by hand ([item format](docs/format.mara.md)) |
| Check the project                   | `mara project validate`                                                                             |
| Find and read items                 | `mara search <query>`, `mara get <ID>`                                                              |
| Give an agent the workflow          | `mara prime`, or the MCP server `mara mcp --project <root>` ([agent guidance](docs/agents.mara.md)) |
| See every operation                 | `mara`                                                                                              |

## Develop

| To                                  | Go to                                                                                                           |
| ----------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Read the specification              | [docs/index.mara.md](docs/index.mara.md)                                                                        |
| Find work                           | `zban list --ready`, or the Markdown issues in `.zban/`: one epic per release slice                             |
| Contribute, as a person or an agent | [AGENTS.md](AGENTS.md)                                                                                          |

## Licence

MIT or Apache-2.0, at your option.
