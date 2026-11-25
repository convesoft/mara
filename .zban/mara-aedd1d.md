---
title: "The browser module's boundary: plain WebAssembly exports, or wasm-bindgen through nx-rust's wasm-build"
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
discovered_from: mara-jptfbq
created_at: 2026-11-25T11:40:26Z
---

Goal: the maintainer confirms or overturns how the browser's WebAssembly module is built and called.

Context: mara-jptfbq compiles the `mara` library's host code for `wasm32-unknown-unknown` in the crate `mara-wasm`, which exports four plain functions; `packages/mara-browser/worker.js` passes each call as a JSON header plus file bytes (`ADR-RAW-WASM-MODULE`, docs/hosts/index.mara.md). The worker rejected `wasm-bindgen` through the `wasm-build` target of `@convesoft/nx-rust`, `wasm-pack` and Emscripten. A second opinion (Gemini, 2026-11-25) recommends keeping the plain boundary: it is about 100 lines on both sides, and `wasm-bindgen` needs a `wasm-bindgen-cli` binary of exactly the crate's version in every developer's environment, in CI and in the release. It found four defects in the hand-written boundary (an allocation that relied on an allocator detail, an unchecked length conversion, signed pointers in JavaScript, a caller error that stopped the workspace), which mara-jptfbq fixes, and names what would change the choice: a larger two-way surface between JavaScript and Rust in mara-xsm8w6, or the wish to use the plugin's own target here. One finding about the plugin, reported and not patched: for a `cdylib` built for `wasm32-unknown-unknown` it declares the outputs `libX.so` and `libX.dylib` and misses `X.wasm`; the crate names the module in its target table instead.

1. Keep the plain exports, hardened (recommended; provisional until answered).
2. Build the module with `wasm-bindgen` through nx-rust's `wasm-build`, and pin `wasm-bindgen-cli` beside cargo-nextest in CI and the release. Undo of option 1: one issue that replaces `crates/mara-wasm/src/lib.rs` and the framing in `worker.js`; the workspace and its tests stay.

- [ ] The maintainer has answered
