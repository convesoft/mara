---
title: Lint the WebAssembly build of the browser workspace
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-jptfbq
created_at: 2026-11-25T11:22:22Z
---

The local checks lint the crates natively only (`cargo clippy --workspace --all-targets`). Code that compiles only for `wasm32-unknown-unknown` is never linted: `crates/mara-wasm/src/lib.rs`, `crates/mara/src/cli/workspace.rs`, `crates/mara/src/grammar/browser.rs` and the wasm parts of `crates/mara/src/system.rs` (mara-jptfbq, ADR-RAW-WASM-MODULE and ADR-MEMORY-FILESYSTEM in `docs/hosts/index.mara.md`). `cargo clippy -p mara-wasm --target wasm32-unknown-unknown` passes for those today, but reports one existing warning in `mara-engine` that the native lint does not see (`clippy::unnecessary_min_or_max` at `crates/mara-engine/src/document.rs:325`, where `usize` is 32 bits).

- [ ] Decide whether a `lint` configuration for `wasm32-unknown-unknown` joins `pnpm run checks` (ADR-NX-RUST-TASKS), and what it costs
- [ ] Fix what it reports
