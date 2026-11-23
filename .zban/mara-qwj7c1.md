---
title: Named Nx inputs for the test hooks and Soufflé, and pack-native finds Soufflé where cargo's artifact directory is
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-6z7bjs
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: the inputs that `mara:test` and `mara:validate` share are declared once, and `pack-native` reads Soufflé from the artifact directory it reads `mara` from.

Context: architecture audit after rc.1. `crates/mara/Cargo.toml` repeats the same eleven `MARA_TEST_*` environment inputs and the Soufflé runtime command for `test` and `validate`; a new hook in `crates/mara/src/hooks.rs` needs both edited. `mara:validate` hashes the whole staged Soufflé folder through `dependentTasksOutputFiles = "**/*"` and the binary again through its runtime command (review of mara-jmt2hr). `packages/mara-npm/project.json` passes `target/souffle-2.5/souffle` and its `LICENSE` as fixed paths, while `pack.mjs` takes `mara` from cargo's artifact directory. Related: mara-6z7bjs (where Soufflé is under another `CARGO_TARGET_DIR`).

- [ ] Shared named inputs for the hooks and the Soufflé key, used by both targets
- [ ] `pack-native` resolves Soufflé from the artifact directory, in step with mara-6z7bjs
- [ ] `mara:validate`'s dependency-output glob is narrowed to what it reads, once mara-6z7bjs covers the lookup paths
