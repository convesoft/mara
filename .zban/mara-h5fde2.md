---
title: Licence notices for C/C++ runtimes in bundled Soufflé (Emscripten musl/libc++ and static libstdc++)
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
discovered_from: mara-865a2e
created_at: 2026-11-25T18:30:35Z
---

Goal: decide whether `THIRD-PARTY-LICENSES` should also include notices for Emscripten's C/C++ runtime (musl, libc++) in `souffle.wasm`/`souffle.js` and static `libstdc++` in native `bin/souffle`.

Context: `mara-865a2e` generates `THIRD-PARTY-LICENSES` for all Rust crates compiled into `mara.wasm` (in `@convesoft/mara`) and into `mara` + `mara-node` (in the Linux native packages), matching `DES-PACKAGES` and `mara-6qcfn5`. `souffle.wasm`/`souffle.js` also embed Emscripten's runtime (musl/libc++), and native `bin/souffle` links static `libstdc++`, neither of which currently has a separate notice beyond `LICENSE-SOUFFLE` (UPL-1.0).

Options:
- [ ] Check which runtime licences require attribution in binary distributions and add them to `THIRD-PARTY-LICENSES` before 0.4.0 (recommended; provisional: ship rc.2 with current Rust + Soufflé notices).
- [ ] Keep `THIRD-PARTY-LICENSES` scoped to Rust crates and `LICENSE-SOUFFLE` only.
