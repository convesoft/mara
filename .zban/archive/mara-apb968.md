---
title: Lint the WebAssembly build of the browser workspace
status: done
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-jptfbq
created_at: 2026-11-25T11:22:22Z
closed_at: 2026-11-25T11:50:12Z
---

The local checks lint the crates natively only (`cargo clippy --workspace --all-targets`). Code that compiles only for `wasm32-unknown-unknown` is never linted: `crates/mara-wasm/src/lib.rs`, `crates/mara/src/cli/workspace.rs`, `crates/mara/src/grammar/browser.rs` and the wasm parts of `crates/mara/src/system.rs` (mara-jptfbq, ADR-RAW-WASM-MODULE and ADR-MEMORY-FILESYSTEM in `docs/hosts/index.mara.md`). `cargo clippy -p mara-wasm --target wasm32-unknown-unknown` passes for those today, but reports one existing warning in `mara-engine` that the native lint does not see (`clippy::unnecessary_min_or_max` at `crates/mara-engine/src/document.rs:325`, where `usize` is 32 bits).

- [x] Decide whether a `lint` configuration for `wasm32-unknown-unknown` joins `pnpm run checks` (ADR-NX-RUST-TASKS), and what it costs
- [x] Fix what it reports

## Summary of Changes

Done in mara-jptfbq (fix round 1 of pull request #85): the configuration `wasm32` of `mara-wasm:lint` (`targetTriple = "wasm32-unknown-unknown"`, `allTargets = false`) lints `mara-wasm`, the `mara` library and the engine as the module builds them, and `pnpm run checks` runs it as `nx run mara-wasm:lint:wasm32` after `mara-engine:build:wasm32`; `checks:fast` leaves it out. It reported the engine's `clippy::unnecessary_min_or_max` at `document.rs`, where `MAX_DOCUMENT_BYTES` is `usize`'s maximum on 32 bits (allowed there with that reason), and two warnings of the wasm-only code in `tracked.rs`, fixed. ADR-NX-RUST-TASKS and AGENTS.md "Local checks" name it.
