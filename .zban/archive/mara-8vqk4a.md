---
title: MCP server with CLI parity
status: done
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-w0xasr
  - mara-esxqzv
  - mara-pvqpse
  - mara-sj9jfq
  - mara-wjy3sh
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-16T11:55:19Z
---

Context: docs/hosts.mara.md (REQ-CLI-MCP-PARITY, DES-OPERATIONS).

- [x] Confirm the operation names with the maintainer and set DES-OPERATIONS to Accepted (ADR-GROUP-VERB-COMMANDS)
- [x] Stdio MCP server exposing every document operation; project per call or bound at start
- [x] VER-CLI-MCP-PARITY passes (docs/verification-hosts.mara.md), and the MCP case of VER-CONCURRENT-WRITERS (docs/verification-writes.mara.md)
- [x] Work-counter and scaling cases for `mcp` (DES-PERFORMANCE-HARNESS)

## Summary of Changes

- `mara mcp [--project <dir>]` (`crates/mara/src/cli/mcp.rs`): a stdio MCP server on `rmcp` 3.10.0 (features `server` and `transport-io`, default features off) and `tokio` 1.54.1 (feature `rt`), both in the `mara` crate only. `cargo tree -p mara-engine` holds neither, and the engine still builds for `wasm32-unknown-unknown`. The single-threaded runtime starts only in `serve`; every other operation stays synchronous with unchanged counts.
  - `rmcp` owns the protocol: the `initialize` handshake of 2024-11-05 to 2025-11-25, the 2026-07-28 revision (`server/discover`, per-request `_meta`), version negotiation, `ping`, error shapes, cancellation and stdio framing.
  - `list_tools` builds one tool per document operation from the CLI's one list (`OPERATIONS`). `call_tool` builds the operation's `Args` from the call's arguments and runs the same function as the CLI, on the runtime's blocking pool. The tool's text is the JSON the CLI prints with `--format json`, with `isError` for a failure.
  - `instructions()` is where mara-18193r supplies the agent guide; it returns `None` until then.
  - `--debug-stats` prints one stats line at exit: the work of every call of the session.
- `crates/mara/src/cli.rs` and the operation modules, restructured once for all operations: `Operation` gains `summary` and typed `arguments` (`Argument { name, help }`), `Opt` gains `help`, `required` and `integer`, and `run` is a `Run`: `Document` (a request and its result, also an MCP tool) or `Serve` (`mcp`). CLI behaviour and output are unchanged except the new `mara mcp` line in the usage; every existing CLI test passes unmodified.
- Tests:
  - `crates/mara/tests/mcp/` starts the built binary and speaks JSON-RPC to it over stdio. VER-CLI-MCP-PARITY: the tool list against the CLI's usage, both directions; an agent's workflow from `project init` through create, relate, update, get, search, list, move, rename, hand edits (a document moved by hand, an item written without an anchor), `fix` and a clean `validate`, and failing requests (`unknown-item`, `unknown-node`, `unknown-declaration`, `targeted-item`, `inverse-relation`, `not-an-input`, `invalid-position`, `missing-relation`, `wrong-id-prefix`, `not-a-legacy-project`, `existing-file`, `invalid-project`, `invalid-argument` from inside an operation). Each step goes through the CLI and MCP from the same files at the same path, with generated MIDs paired by path and line: same JSON text and same files. Also a 0.3.0 conversion; a project per call (absolute and relative) and a bound server; undeclared, missing and mistyped parameters rejected with no work and no file change; a client of the 2026-07-28 revision gets the same tools and results.
  - VER-CONCURRENT-WRITERS, MCP case: a CLI `item create` and an MCP `item_create` at once, three rounds; and an MCP write waits for the write lock the test holds, then applies to the changed file.
  - `crates/mara/tests/performance/mcp.rs`: `mara mcp` cases, one server session each (handshake, `tools/list`, one call). Counters: a session without a call does no work; `project_validate`, `item_list`, `schema_get` and `item_create` have exactly the cold counts of the same CLI requests (`check_cold`, now shared by the CLI and MCP cases). Scaling: the session's start-up and each call.
- Docs: DES-MCP-SERVER and ADR-RMCP-SERVER on `docs/hosts.mara.md`, a link to DES-MCP-SERVER from DES-OPERATIONS, and one bullet on `mcp` cases in DES-PERFORMANCE-HARNESS.
- By hand: the official MCP Inspector CLI (TypeScript SDK) lists the tools, and creates and lists an item through `mara mcp`.

Checks, all exit 0:
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`
- `cargo test --workspace --locked` (the `mcp` binary: 10 tests)
- `cargo run --package mara --locked --quiet -- project validate` (valid, 0 errors, 0 warnings)
- `cargo test --package mara --test performance --release --locked`
- `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`
- `zban check`

## Decisions made

- Decided by the maintainer: "yes lets rely on rmcp so we don't have to maintain all the edges ourselves". The server is built on `rmcp`; rejected: the hand-written JSON-RPC server on `serde_json` that this branch had first (ADR-RMCP-SERVER).
- Protocol versions: `rmcp`'s default set, both eras. Rejected: narrowing it, which would refuse clients that `rmcp` serves.
- Parameters (DES-MCP-SERVER): named exactly as the CLI's arguments and options (`id-or-mid`, `clear-field`); a positional argument is a required string; an option a string, an integer for `limit` and `line`, an array when it repeats; `title` and `body` of `item create` listed as required (the operation still refuses a request without them, with the CLI's message); `null` counts as left out, for clients that send every parameter; `additionalProperties: false`.
- Project: a server without `--project` adds a `project` parameter to each tool, absolute or relative to the server's working directory, and without it finds the project from its working directory upward, as the CLI does. A server with `--project` lists no `project` parameter and rejects one with `invalid-argument`.
- Results: one text content block with the CLI's JSON, `isError` for a failure, no structured content or output schema. A rejection that the server makes before the operation runs (undeclared, missing or mistyped parameter) is `invalid-argument` with a message in MCP's terms; everything the operation decides is byte-identical to the CLI. An unknown tool is a JSON-RPC error (-32602).
- Concurrency: calls run on the blocking pool, several at once when the client sends them so; writes wait for the write lock. A cancelled call runs to its end and `rmcp` drops its response (one sentence in DES-MCP-SERVER).
- `--debug-stats` on the server: one stats line at exit, the sum of its calls. A performance case is one session with one call.
- `mcp` is an operation in the CLI's list and usage, and not a tool (`Run::Serve`).
