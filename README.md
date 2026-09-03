# Mara

Mara keeps project knowledge in readable Markdown while giving requirements,
designs, decisions, and other durable facts stable identities, types, relations,
validation, and deterministic retrieval. The same operations are available as
a CLI and a stdio MCP server.

Mara is currently pre-release software. The supported alpha hosts are x64 and
arm64 macOS, plus x64 and arm64 Linux compatible with Ubuntu 22.04's glibc
baseline.

## Run with npx

Pin the exact version so an MCP restart cannot silently change behavior:

```bash
npx -y @convesoft/mara@0.1.0-alpha.0 --version
npx -y @convesoft/mara@0.1.0-alpha.0 project init ./example
npx -y @convesoft/mara@0.1.0-alpha.0 --project ./example project validate
```

The npm packages contain prebuilt native binaries and use no install scripts.
A Rust toolchain is not required.

## Configure an MCP client

For a client that starts stdio servers in the project directory:

```toml
[mcp_servers.mara]
command = "npx"
args = ["-y", "@convesoft/mara@0.1.0-alpha.0", "mcp"]
```

To bind the server to one project regardless of its execution directory, place
`--project` after `mcp`:

```toml
[mcp_servers.mara]
command = "npx"
args = [
  "-y",
  "@convesoft/mara@0.1.0-alpha.0",
  "mcp",
  "--project",
  "/absolute/path/to/project",
]
```

Without `--project`, the server can start anywhere. Project-bound tools accept
an absolute `project` path or discover the nearest parent containing
`.mara/project.toml` from the server's execution directory.

## Agent Plugin package

The main npm package also contains a portable Agent Plugins 1.0 manifest, a
Mara skill, and stdio MCP configuration. Compatible clients can install that
package through their supported plugin distribution flow. Codex is the
reference client; the portable package does not modify project `AGENTS.md`.

Starting with `0.1.0-alpha.1`, a user without an existing Mara installation can
install the complete package from the Convesoft Codex marketplace:

```bash
codex plugin marketplace add convesoft/mara
codex plugin add mara@convesoft
```

Start a new Codex session after installation. Codex keeps an installed plugin
snapshot in its managed cache. On first MCP start, its launcher uses `npx` to
install and run that snapshot's exact Mara version with the matching native
package in npm's cache.

If Mara and its MCP server are already configured, install only the skill and
keep using that existing executable:

```bash
npx skills add convesoft/mara --skill mara -g -a codex
```

For a new MCP registration backed by an existing installation, use the
executable's absolute path:

```bash
codex mcp add mara -- /absolute/path/to/mara mcp
```

Use either the complete plugin or the existing-installation route. Do not add
the complete plugin alongside an equivalent manually configured Mara MCP
server.

## Core workflow

```bash
mara project init
mara schema get
mara item create requirement REQ-EXAMPLE docs/example.mara.md \
  --title "State one verifiable obligation" \
  --body "The project must demonstrate its primary workflow."
mara project validate
mara item search "primary workflow"
mara item get REQ-EXAMPLE
```

Run `mara --help` or `mara <object> <operation> --help` for the complete command
surface. The canonical alpha behavior is documented in
[`docs/alpha.mara.md`](docs/alpha.mara.md); distribution and release guarantees
are in [`docs/distribution.mara.md`](docs/distribution.mara.md).

## Development

The repository pins its Rust toolchain. From a checkout:

```bash
cargo test --locked --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo run -- --format json project validate
```

See [`ROADMAP.md`](ROADMAP.md), [`AGENTS.md`](AGENTS.md), and
[`SECURITY.md`](SECURITY.md).

## License

Licensed under either [Apache License 2.0](LICENSE-APACHE) or the
[MIT License](LICENSE-MIT), at your option.
