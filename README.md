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
npx -y @convesoft/mara@0.1.0-alpha.1 --version
npx -y @convesoft/mara@0.1.0-alpha.1 project init ./example
npx -y @convesoft/mara@0.1.0-alpha.1 --project ./example project validate
```

The npm packages contain prebuilt native binaries and use no install scripts.
A Rust toolchain is not required.

## Configure an MCP client

For a client that starts stdio servers in the project directory:

```toml
[mcp_servers.mara]
command = "npx"
args = ["-y", "@convesoft/mara@0.1.0-alpha.1", "mcp"]
```

To bind the server to one project regardless of its execution directory, place
`--project` after `mcp`:

```toml
[mcp_servers.mara]
command = "npx"
args = [
  "-y",
  "@convesoft/mara@0.1.0-alpha.1",
  "mcp",
  "--project",
  "/absolute/path/to/project",
]
```

Without `--project`, the server can start anywhere. Project-bound tools accept
an absolute `project` path or discover the nearest parent containing
`.mara/project.toml` from the server's execution directory.

## Configure Codex

Register the installed Mara executable as an MCP server and install the Mara
skill separately:

```bash
codex mcp add mara -- npx -y @convesoft/mara@0.1.0-alpha.1 mcp
npx skills add convesoft/mara --skill mara -g -a codex
```

If Mara is already installed, register its absolute executable path instead:

```bash
codex mcp add mara -- /absolute/path/to/mara mcp
```

The skill and MCP server expose the same Mara operations without installing a
second executable or depending on a client's plugin-cache layout.

The npm package also contains an optional portable Agent Plugins 1.0 manifest,
skill, and MCP configuration. Compatible clients may install the complete
package through the Convesoft marketplace as a convenience:

```bash
codex plugin marketplace add convesoft/mara
codex plugin add mara@convesoft
```

The complete plugin is not a release compatibility target. Do not install it
alongside an equivalent manually configured Mara MCP server. Neither onboarding
route modifies project `AGENTS.md`.

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
