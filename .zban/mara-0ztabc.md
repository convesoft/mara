---
title: Scaffold the repository
status: todo
type: task
parent: mara-t1rffa
created_at: 2026-11-13T22:10:06Z
---

Goal: an empty workspace that builds, tests and checks itself in CI.

Context: AGENTS.md (Engineering); DES-ENGINE-BOUNDARY in docs/engine.mara.md.

- [ ] Cargo workspace with an engine crate that does no I/O and a CLI crate; `Cargo.lock` committed; toolchain pinned
- [ ] `LICENSE-MIT` and `LICENSE-APACHE` restored (the texts are in `git show d7f0e73:LICENSE-MIT` and `:LICENSE-APACHE`)
- [ ] CI on Linux runners only (macOS waits for mara-ag4ees): format, lint, tests, and a `wasm32` build of the engine crate
