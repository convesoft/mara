---
title: Set the workspace version to 0.4.0-alpha.3
status: done
type: task
discovered_from: mara-17tdb3
created_at: 2026-11-20T15:38:09Z
closed_at: 2026-11-20T15:51:53Z
---

Goal: the binary built from the commit that is tagged `v0.4.0-alpha.3` reports version `0.4.0-alpha.3`.

Context: the alpha.3 epic (mara-17tdb3) is closed and the slice can be tagged, but the one workspace version in `Cargo.toml` still says `0.4.0-alpha.2` (AGENTS.md: "keep one workspace version"; "Tag releasable slices with annotated `vX.Y.Z[-prerelease]` tags"). The MCP server reports that version in `serverInfo`, the daemon compares builds, and `Cargo.lock` records it for the two workspace packages. A trivial chore under the maintainer's rule of 2026-11-16: no reviewer; the orchestrator checks the result. mara-792nvv did the same for alpha.2.

Done when `cargo run --package mara --locked --quiet -- --no-daemon mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.3`, the local checks of AGENTS.md pass, and the diff holds nothing but the version.

- [x] `version` in the workspace `Cargo.toml` is `0.4.0-alpha.3`, and `Cargo.lock` follows
- [x] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-alpha.2` as a past slice stay

## Summary of Changes

- `Cargo.toml` workspace version and the two workspace entries of `Cargo.lock` are `0.4.0-alpha.3`. No test or doc sentence states the current version as a fact (tests use `CARGO_PKG_VERSION`; `docs/verification.mara.md` names slices).
- `mara --no-daemon mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.3`.
- Checks: fmt, clippy, workspace tests, project validate, release performance test, wasm32 build, zban check all exit 0.

## Decisions made

None.
