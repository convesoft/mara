---
title: Set the workspace version to 0.4.0-alpha.2
status: done
type: task
discovered_from: mara-6k50w0
created_at: 2026-11-16T15:27:18Z
closed_at: 2026-11-16T15:35:26Z
---

Goal: the binary built from the commit that is tagged `v0.4.0-alpha.2` reports version `0.4.0-alpha.2`.

Context: the alpha.2 epic (mara-6k50w0) is closed and the slice can be tagged, but the one workspace version in `Cargo.toml` still says `0.4.0-alpha.1` (AGENTS.md: "keep one workspace version"; "Tag releasable slices with annotated `vX.Y.Z[-prerelease]` tags"). The MCP server reports that version in `serverInfo`, and `Cargo.lock` records it for the two workspace packages. A trivial chore under the maintainer's rule of 2026-11-16: no reviewer; the orchestrator checks the result.

Done when `cargo run --package mara --locked --quiet -- mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.2`, the local checks of AGENTS.md pass, and the diff holds nothing but the version.

- [x] `version` in the workspace `Cargo.toml` is `0.4.0-alpha.2`, and `Cargo.lock` follows
- [x] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-alpha.1` as a past slice stay

## Summary of Changes

- Workspace `version` in `Cargo.toml` is `0.4.0-alpha.2`; `Cargo.lock` changed only in the `mara` and `mara-engine` entries.
- No test or doc states the current version; mentions of `0.4.0-alpha.1` as a past slice or measurement stay.
- `mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.2`.
- Checks: `cargo fmt --all --check`, `cargo test --workspace --locked`, `project validate`, `zban check` pass.

## Decisions made

None.
