---
title: Scaffold the repository
status: in-progress
type: task
parent: mara-t1rffa
created_at: 2026-11-13T22:10:06Z
---

Goal: an empty workspace that builds, tests and checks itself locally.

Context: AGENTS.md (Engineering); DES-ENGINE-BOUNDARY in docs/engine.mara.md.

- [ ] Cargo workspace with an engine crate that does no I/O and a CLI crate; `Cargo.lock` committed; toolchain pinned
- [ ] `LICENSE-MIT` and `LICENSE-APACHE` restored (the texts are in `git show d7f0e73:LICENSE-MIT` and `:LICENSE-APACHE`)
- [ ] Local checks, with their commands listed in AGENTS.md: format, lint, tests, and the engine crate's `wasm32` build that VER-HOST-COMPARISON (docs/verification.mara.md) requires (CI waits for mara-4dr2p2)
