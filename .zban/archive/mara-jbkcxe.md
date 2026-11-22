---
title: Set the workspace version to 0.4.0-rc.1
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-1avrq4
  - mara-e8921x
created_at: 2026-11-22T20:51:00Z
closed_at: 2026-11-22T22:53:48Z
---

Goal: the binary and the packages built from the commit the release workflow publishes report version `0.4.0-rc.1`.

Context: `ADR-DISPATCHED-RELEASE` and `DES-RELEASE-WORKFLOW` (docs/distribution.mara.md) set the version by hand in `Cargo.toml`; the workspace version still says `0.4.0-alpha.5`, which `mara --version` and `serverInfo.version` print. AGENTS.md: "keep one workspace version". mara-g2acnm did the same for alpha.5. A trivial chore: no reviewer; the orchestrator checks the result. It waits for mara-1avrq4 (the root `Cargo.toml`) and mara-e8921x (`mara --version`).

- [x] `version` in the workspace `Cargo.toml` is `0.4.0-rc.1`, and `Cargo.lock` follows for every workspace package
- [x] Anything that states the current version as a fact about the build (a test, a doc sentence) follows; mentions of `0.4.0-alpha.5` as a past slice stay
- [x] `mara --version` prints `mara 0.4.0-rc.1`, the local checks pass, and the diff holds nothing but the version

## Summary of Changes

- `Cargo.toml` workspace `version` is `0.4.0-rc.1`; `Cargo.lock` follows for `mara` and `mara-engine` (`cargo update --workspace --offline`).
- No test or doc states the current version; the `0.4.0-alpha.5` mentions in `docs/` and `.zban/` name the past slice.
- `cargo run --quiet --bin mara -- --version` prints `mara 0.4.0-rc.1` (`--no-daemon --version` is refused: `--version` takes no other argument).
- Checks: `pnpm run checks` exit 0; `project validate` 0 errors, 0 warnings; 860 passed, 0 failed, 16 ignored; wasm32 build of `mara-engine` and `zban check` pass.

## Decisions made

- None.
