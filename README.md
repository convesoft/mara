# Mara

Mara keeps structured project knowledge in readable, Git-tracked Markdown.
The CLI and stdio MCP server share authoring, retrieval, validation and
traceability operations. Start with the [product documentation](docs/index.mara.md)
and [Mara skill](skills/mara/SKILL.md).

## Run

Build from this checkout with the pinned Rust toolchain:

```bash
cargo build --locked --release
./target/release/mara --help
```

For a published release, replace `<version>` with its exact version:

```bash
npx -y '@convesoft/mara@<version>' --help
```

The npm package runs a prebuilt binary without install scripts or a Rust toolchain.
Supported platforms and package contents are defined in
[distribution](docs/distribution.mara.md).
For existing projects and clients, follow the [0.3 migration guide](docs/migration-0.3.mara.md).

## Configure an agent

Configure your MCP client to launch `/absolute/path/to/mara` with arguments
`["mcp", "--project", "/absolute/path/to/project"]`. For npm, launch `npx` with
`["-y", "@convesoft/mara@<version>", "mcp", "--project", "/absolute/path/to/project"]`.
Use the same exact version for CLI and MCP. A bound server uses that project;
omit per-call project overrides.

Install the standalone skill separately from the matching checkout or extracted
npm package:

```bash
npx skills add /absolute/path/to/mara-package --skill mara
```

Select the intended agent and installation scope. The skill installer supports
[local sources and skill selection](https://github.com/vercel-labs/skills#install-a-skill).
Installing the skill does not install Mara; its CLI fallback reuses the configured
MCP executable or exact npm pin.

## Author knowledge

In a new project directory, using `mara` for the selected executable:

```bash
mara project init --template engineering
mara schema get flavour requirement
mara item create requirement REQ-ACCESS knowledge.mara.md \
  --title 'Permit access' --body 'An authorized user can access the service.' \
  --field status=draft
mara project validate
mara --format json search 'authorized user'
mara --format json get REQ-ACCESS
```

Inspect the schema before choosing a flavour or relation. See
[initialization and project context](docs/project.mara.md),
[retrieval](docs/retrieval.mara.md), and [trace matrices](docs/traceability.mara.md)
for their full contracts.

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE).
