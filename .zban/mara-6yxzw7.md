---
title: Set the workspace version to 0.4.0-alpha.4
status: in-progress
type: task
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:12:31Z
---

Goal: the binary built from the commit that is tagged `v0.4.0-alpha.4` reports version `0.4.0-alpha.4`.

Context: the alpha.4 epic (mara-0mxw21) is closed and the slice can be tagged, but the one workspace version in `Cargo.toml` still says `0.4.0-alpha.3` (AGENTS.md: "keep one workspace version"; "Tag releasable slices with annotated `vX.Y.Z[-prerelease]` tags"). The MCP server reports that version in `serverInfo`, the daemon compares builds, and `Cargo.lock` records it for the two workspace packages. A trivial chore: no reviewer; the orchestrator checks the result. mara-4vmx1k did the same for alpha.3.

Done when `cargo run --package mara --locked --quiet -- --no-daemon mcp` answers `initialize` with `serverInfo.version` `0.4.0-alpha.4`, the local checks of AGENTS.md pass, and the diff holds nothing but the version.

- [ ] `version` in the workspace `Cargo.toml` is `0.4.0-alpha.4`, and `Cargo.lock` follows
- [ ] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-alpha.3` as a past slice stay
