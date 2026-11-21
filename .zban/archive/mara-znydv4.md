---
title: Pin the Rust grammar in this repository and scan every source file
status: done
type: task
related:
  - mara-006gqt
discovered_from: mara-cp1skx
created_at: 2026-11-21T21:55:03Z
closed_at: 2026-11-21T22:02:54Z
---

Goal: this repository reads its own code markers at the grammar level, and `[code]` in `.mara/config.toml` names the crates' source with two patterns instead of fifteen.

Context: mara-cp1skx (commit `b561961`) declared Rust at the basic level, by the orchestrator's provisional decision, and listed the engine's source directories one by one, because at the basic level the marker-shaped test strings of `crates/mara-engine/src/code/tests.rs` are read as markers and patterns cannot exclude a file. At the grammar level only text inside comment nodes can hold a marker (`ScannedFile::scan_syntax`, `crates/mara-engine/src/code.rs`; DES-CODE-MARKERS in docs/code/index.mara.md), so those strings are no markers and the file needs no exclusion.

Decided by the maintainer on 2026-11-21, in the orchestrator's thread: "Yes please pin grammar and fix globs". The maintainer authorised one run of `mara code add rust` with the binary built from this repository, against the real per-user cache (`~/.cache/mara/grammars`): it downloads the two files of the shipped catalog's rust 0.24.2 from GitHub and verifies them against the catalog's checksum. This replaces the provisional decision of mara-cp1skx recorded in mara-7ztypp.

Consequence the maintainer accepted: on a machine whose cache lacks the grammar, `project validate`, one of the local checks, fails with `unavailable-grammar` until `mara code add rust` has run there once (REQ-ONLY-CODE-ADD-DOWNLOADS).

Done when `cargo run --package mara --locked --quiet -- --no-daemon project validate` reports 0 errors and no warning but the known `requirement-verified` one, `query execute implemented --limit 100` lists the same 94 links on 43 requirements, each with a declaration where its marker sits above one, and the seven local checks pass.

- [x] `mara code add rust` has pinned rust 0.24.2 in `.mara/config.toml` with the catalog's checksum, and the grammar is in the per-user cache
- [x] `[code.languages.rust]` includes `crates/mara/src/**/*.rs` and `crates/mara-engine/src/**/*.rs` and nothing else; no marker diagnostic appears, and no file under either `src` is left unscanned
- [x] AGENTS.md: the line on markers no longer tells authors to check the patterns for a new engine directory, and "Local checks" says that a machine runs `mara code add rust` once before them
- [x] The second item of mara-006gqt is ticked or removed, with a note that this issue did it

## Summary of Changes

- `mara code add rust` (worktree binary, real cache) pinned rust 0.24.2, `blake3:f03b54d5fcef88042d4645c2820c4a4bedbd4edaed2e01ad85e67b325c2d7149`, in `[code.languages.rust]` of `.mara/config.toml`; it changed neither the patterns nor the prefixes. The grammar is in `~/.cache/mara/grammars/rust`.
- `[code.languages.rust]` includes `crates/mara/src/**/*.rs` and `crates/mara-engine/src/**/*.rs`; its comment says why `crates/*/tests/` stays out.
- AGENTS.md: the marker line says every `.rs` file under both `src` directories is scanned at the grammar level; "Local checks" says a machine runs `code add rust` once first (DES-GRAMMAR-ASSETS).
- mara-006gqt: second item ticked, with a note.
- `project validate`: 0 errors, only the known `requirement-verified` warning; `code_scans` 110, the number of `.rs` files under both `src`. `implemented`: the same 94 links on 43 requirements; 93 now name the function under their marker, the one in `crates/mara/src/cli.rs` (above `OPERATIONS`) belongs to the file. No marker belongs to another declaration than the one it sits above.
- `project validate` time (hyperfine, 3 runs): debug 155 ms to 1.98 s, release 60 ms to 312 ms; the grammar-level scan costs this.
- Checks: fmt, clippy, the workspace tests, `project validate`, the release performance test (32 passed), the wasm32 build and `zban check` all exit 0.

## Decisions made

- None beyond the wording of the configuration comment.
