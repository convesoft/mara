---
title: Set the workspace version to 0.4.0-alpha.5
status: done
type: task
discovered_from: mara-779tms
created_at: 2026-11-21T21:20:23Z
closed_at: 2026-11-21T21:31:33Z
---

Goal: the binary built from the commit that is tagged `v0.4.0-alpha.5` reports version `0.4.0-alpha.5`.

Context: the alpha.5 epic (mara-779tms) is closed and the slice can be tagged, but the one workspace version in `Cargo.toml` still says `0.4.0-alpha.4` (AGENTS.md: "keep one workspace version"; "Tag releasable slices with annotated `vX.Y.Z[-prerelease]` tags"). The MCP server reports that version in `serverInfo`, the daemon compares builds, and `Cargo.lock` records it for the two workspace packages. A trivial chore: no reviewer; the orchestrator checks the result. mara-6yxzw7 did the same for alpha.4.

Done when `cargo run --package mara --locked --quiet -- --no-daemon mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.5`, the local checks of AGENTS.md pass, and the diff holds nothing but the version.

- [x] `version` in the workspace `Cargo.toml` is `0.4.0-alpha.5`, and `Cargo.lock` follows
- [x] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-alpha.4` as a past slice stay

## Summary of Changes

- `Cargo.toml` workspace `version` is `0.4.0-alpha.5`; `Cargo.lock` follows for `mara` and `mara-engine` (`cargo update --workspace --offline`).
- No test or doc states the current version; the `0.4.0-alpha.4` mentions left in issues name the past slice.
- `mcp` `initialize` reports `serverInfo.version` `0.4.0-alpha.5`.
- Checks: fmt, clippy (warnings denied), `cargo test --workspace`, `project validate`, release performance test, wasm32 build of `mara-engine`, `zban check`: all exit 0.

## Decisions made

- None.
