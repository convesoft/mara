---
title: Diagnostic codes and paths are not leaked for the life of the process
status: done
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T10:03:48Z
---

Goal: a long-lived process (the daemon, `mara mcp`) gives back the memory of the strings its diagnostics name when the session that produced them is dropped.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. `intern` in `crates/mara-engine/src/diagnostic.rs` (line 105) turns every distinct string it is given into a `&'static str` with `Box::leak` and keeps it in a process-wide table. In a one-shot CLI call that is harmless. Since daemon sessions (mara-xqcnqw, `2af3de7`) one process serves many projects for a long time, and every path or label it ever reported stays allocated until it exits. Items: DES-DAEMON-SESSIONS and DES-IDLE-SESSIONS (docs/daemon/index.mara.md), DES-DIAGNOSTICS (docs/validation.mara.md); the engine does no I/O and builds for `wasm32` (docs/engine.mara.md).

Done when `diagnostic.rs` has no `Box::leak`, a dropped session frees the strings of its diagnostics, and the local checks of AGENTS.md pass, the `wasm32` build included.

- [x] Diagnostics hold their strings without leaking them (`Arc<str>`, an owned `String` or `Cow<'static, str>`), with no change to any result or work count
- [x] A test shows that the memory of a dropped session's diagnostic strings is released, or that no interning table grows with the number of distinct paths

## Summary of Changes

- `Diagnostic::code` is a `Cow<'static, str>`: borrowed for a built-in check, owned for an invariant's name. `intern` and its process-wide `Box::leak` table are gone from `crates/mara-engine/src/diagnostic.rs`; `invariant_findings` puts a copy of the entry's name in each diagnostic, so dropping a session's results frees it.
- Paths, messages, IDs and MIDs were already owned `String`s; only codes were interned.
- `Diagnostic::error` takes `impl Into<Cow<'static, str>>`, so every caller that passes a literal code is unchanged. Consumers adjusted: `fix.rs` and `write/read_back.rs` key their maps by `Cow`, `ReadError::code` returns `TOO_LARGE` for `TooLarge` (the only code it carries), `convert.rs`'s `Refusal::file` takes the borrowed code. Test helpers use a test-only `Diagnostic::built_in_code`.
- New test `crates/mara-engine/tests/diagnostic_memory.rs`, a binary of its own without the harness and with a counting global allocator: it builds the invariant diagnostics of 1,000 distinct 64-byte names through `invariant_findings`, drops them, and asserts that live bytes return to their level after a warm-up round of the same size. Checked by hand that it fails (113,064 bytes left) when a process-wide table of the names is put back.
- DES-DIAGNOSTICS gains: a diagnostic owns its strings, except the code of a built-in check, which is a constant of the program; a process keeps none of the others once the result carrying the diagnostic is dropped.
- No result, JSON form or work count changed (the whole test suite, the work-counter and scaling tests included, passes unchanged).

Checks: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --config 'build.warnings="deny"'`, `cargo test --workspace --locked`, `mara --no-daemon project validate` (valid, 1 warning that predates this change), `cargo test --package mara --test performance --release --locked` (under `berth --exclusive`), `cargo build --package mara-engine --target wasm32-unknown-unknown --locked`, `zban check`: all exit 0.

## Decisions made

- `Cow<'static, str>` over `Arc<str>` or `String`: built-in codes stay allocation-free, and only an invariant's name is copied, once per diagnostic. Recorded in DES-DIAGNOSTICS.
- `Refusal::file` in the 0.3.0 conversion falls back to `invalid-project` if a diagnostic ever carried an owned code; the conversion runs no invariant, so this cannot happen today, and `Failure` keeps its `&'static str` code.
