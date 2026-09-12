# Mara

Mara keeps project knowledge in readable Markdown while giving requirements,
designs, decisions, and other durable facts stable identities, types, relations,
validation, and deterministic retrieval. A CLI and stdio MCP server share the
same operations, including discovery of narrative outside items.

This checkout documents the 0.2 interface: schema format 2 with flavour
selection guidance, an engineering template, and unified `search`, `get`, and
`related`. Start with the [0.2 migration guide](docs/migration-0.2.mara.md) for
an existing project. The [0.1.0 documentation](https://github.com/convesoft/mara/tree/v0.1.0)
describes the older released interface; use documentation and skill from the
same revision as your executable.

## Run the current checkout

Build the implementation described here with the pinned Rust toolchain:

```bash
cargo build --locked --release
./target/release/mara --help
```

The examples below use `mara` to mean this executable or an installed version
that exposes the same interface. Register its absolute path for MCP. The
checkout's version string alone does not establish which unreleased changes
an older published prerelease includes; check its help and matching release
notes before using the 0.2 workflow.

Published npm packages contain prebuilt native binaries and use no install
scripts or Rust toolchain. To use one, replace `<version>` with the exact
published version selected for your project:

```bash
npx -y '@convesoft/mara@<version>' --version
npx -y '@convesoft/mara@<version>' --help
```

Keep that exact pin in CLI and MCP launchers. Supported hosts are x64 and
arm64 macOS, plus x64 and arm64 Linux compatible with Ubuntu 22.04's glibc
baseline. Distribution guarantees are in
[distribution and release](docs/distribution.mara.md).

## Configure an MCP client

For a client that starts stdio servers in the project directory:

```toml
[mcp_servers.mara]
command = "/absolute/path/to/mara"
args = ["mcp"]
```

To bind the server to one project regardless of its execution directory:

```toml
[mcp_servers.mara]
command = "/absolute/path/to/mara"
args = ["mcp", "--project", "/absolute/path/to/project"]
```

For npm, use `command = "npx"` and prepend `"-y"` and
`"@convesoft/mara@<version>"` to the arguments after substituting the exact pin.
Without `--project`, project-bound tools accept an absolute `project` path or
discover the nearest `.mara/project.toml` from the server's execution directory.
A bound server rejects request-level project overrides; omit that parameter.

## Configure Codex

Register the executable and install [the Mara skill](skills/mara/SKILL.md)
separately from the same checkout or release:

```bash
codex mcp add mara -- /absolute/path/to/mara mcp
```

Install the `skills/mara` directory through your client's skill installation
workflow. Installing the skill does not install an executable; it reuses the
configured MCP launcher for CLI fallback. For a published version, the npm
package contains the matching skill as well as optional portable Agent Plugins
1.0 metadata and MCP configuration.

Compatible clients may install the complete package through the Convesoft
marketplace as a convenience:

```bash
codex plugin marketplace add convesoft/mara
codex plugin add mara@convesoft
```

The complete plugin is not a release compatibility target. Do not install it
alongside an equivalent manually configured Mara MCP server. Neither onboarding
route modifies project `AGENTS.md`.

## Start authoring

Run this in a new project directory; `knowledge.mara.md` is created by the
first item operation:

```bash
mara project init --template engineering
mara schema list flavour
mara schema get flavour requirement
mara schema get relation verifies
mara item create requirement REQ-ACCESS knowledge.mara.md \
  --title "Permit access" --body "An authorized user can access the service."
mara item create verification VER-ACCESS knowledge.mara.md \
  --title "Check access" \
  --body "Demonstrate that an authorized user can access the service." \
  --relation verifies=REQ-ACCESS
mara project validate
```

`minimal` remains the default template; `empty` declares no vocabulary.
`engineering` supplies engineering flavours and traceability relations.
Templates create configuration and an editable schema only. Before creating an
item, use the flavour's `description`, `use_when`, `avoid_when`, and
`distinguish_from` to choose appropriate knowledge, then inspect its ID prefix,
body, and field constraints. These guidance keys belong to the schema, not
item metadata. See [guided authoring](docs/guided-authoring.mara.md) for the
schema contract and engineering relation meanings.

## Discovery and reading

```bash
mara --format json search "authorized user"
mara --format json get REQ-ACCESS
mara --format json related REQ-ACCESS --direction incoming --relation verifies
mara --format json get VER-ACCESS
```

CLI and MCP use `search`, `get`, and `related`; MCP get/related take
`{"reference":"REQ-ACCESS"}`. Search returns mixed item, section, and Markdown
block hits with one excerpt each. Pass a hit's `node.reference` to get or
related, then read selected `connections[].neighbour.reference` values.
`node.context.parent` identifies direct structural context. Documents and
sections can be read and navigated without client filesystem access.

Repeat a paginated call with its `next_cursor` until `has_more:false`, keeping
all other inputs unchanged. Get returns consecutive content and ordered item
metadata fragments; search excerpts are only for selection. Search and related
accept `limit`; get does not. Item filters exclude narrative; project-relative
path filters cover all search result kinds. For response fields, relation
namespaces, containment, and handle lifetime, see
[the discovery contract](docs/discovery.mara.md).

Item authoring, list, and validation remain under `item`; relation mutations
write schema-defined item edges. Mentions and containment derive from Markdown.
Editing rejects changes that break or retarget surviving internal links; resolve
reported impacts before retrying. See [item editing](docs/editing.mara.md) and
[Markdown links and mutation safety](docs/discovery.mara.md#item-mutation-and-link-safety).
Use `mara --help` or `mara <command> --help` for command and argument guidance.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo run --locked --quiet -- --format json project validate
scripts/smoke-npm.sh target/release/mara
```

See [the documentation index](docs/index.mara.md), [ROADMAP.md](ROADMAP.md),
[AGENTS.md](AGENTS.md), and [SECURITY.md](SECURITY.md).

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or the
[MIT License](LICENSE-MIT), at your option.
