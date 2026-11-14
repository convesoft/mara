---
title: Scaffold the repository
status: done
type: task
parent: mara-t1rffa
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T13:48:05Z
---

Goal: an empty workspace that builds, tests and checks itself locally.

Context: AGENTS.md (Engineering); DES-ENGINE-BOUNDARY in docs/engine.mara.md.

- [x] Cargo workspace with an engine crate that does no I/O and a CLI crate; `Cargo.lock` committed; toolchain pinned
- [x] `LICENSE-MIT` and `LICENSE-APACHE` restored (the texts are in `git show d7f0e73:LICENSE-MIT` and `:LICENSE-APACHE`)
- [x] Local checks, with their commands listed in AGENTS.md: format, lint, tests, and the engine crate's `wasm32` build that VER-HOST-COMPARISON (docs/verification.mara.md) requires (CI waits for mara-4dr2p2)

## Summary of Changes

- Virtual Cargo workspace (`Cargo.toml`: resolver 3, members `crates/*`, one version `0.4.0-alpha.1`, edition 2024) with `crates/mara-engine` (library, no code, no dependencies) and `crates/mara` (the `mara` binary, empty `main`). `Cargo.lock` committed, `/target/` ignored.
- `rust-toolchain.toml` pins 1.100.0 with clippy, rustfmt and the `wasm32-unknown-unknown` target; rustup installs all of it on first use.
- `LICENSE-MIT` and `LICENSE-APACHE` restored, byte-identical to `d7f0e73`.
- AGENTS.md: `## Local checks` lists the five commands; "Until the rewrite is on `main`" links to it.
- DES-ENGINE-BOUNDARY names the engine crate and its WASM target. README no longer says the repository holds no code.

Checks:

- The five commands in AGENTS.md, extracted from the file and run line by line from the repository root: all exit 0.
- The same on a copy of the committed files with empty `RUSTUP_HOME` and `CARGO_HOME`: rustup installed 1.100.0, clippy, rustfmt and `wasm32-unknown-unknown` from the toolchain file, and all five exit 0.
- Each check fails on a seeded fault, in a copy under `$TMPDIR`: misformatted code (fmt, exit 1); an unused variable, a clippy lint, a warning in a test target and an unused Cargo dependency (lint, exit 101); a failing test (test, exit 101); a manifest change with a stale `Cargo.lock` (`--locked`, exit 101); a `getrandom` 0.3 dependency in the engine, which passes the host lint but fails the `wasm32` build (exit 101).

## Decisions made

- Layout: a virtual workspace with packages `mara-engine` and `mara` (binary `mara`, as in 0.3.0) under `crates/`. Rejected: the CLI as root package (0.3.0, `d7f0e73`), which mixes `src/` and `tests/` with `docs/`.
- Toolchain 1.100.0, the current stable; edition 2024, as in 0.3.0.
- WASM target `wasm32-unknown-unknown` (DES-ENGINE-BOUNDARY): the browser build's target, without WASI, so a dependency on host randomness fails the build. Rejected: `wasm32-wasip1`/`wasip2`, which supply the filesystem and clock imports the engine must not use.
- The lint check denies every warning, Cargo's own included, with `--config 'build.warnings="deny"'`. Rejected: `-- -D warnings`, which misses Cargo lints such as `unused_dependencies`.
- `mara` does not depend on `mara-engine` yet: Cargo 1.100 warns on an unused dependency, so the first issue in which the CLI calls the engine adds it.
