---
title: Scan every Rust file of the crates for code markers with one pattern
status: done
type: task
discovered_from: mara-znydv4
created_at: 2026-11-21T22:24:59Z
closed_at: 2026-11-21T22:36:46Z
---

Goal: `[code]` in `.mara/config.toml` names this repository's Rust code with the one pattern `crates/**/*.rs`, and its comment says nothing untrue.

Context: mara-znydv4 (commit `4498546`) pinned the Rust grammar and left `crates/*/tests/` out, with the comment "its fixture files hold real comments with markers of test IDs". That is wrong; the orchestrator gave the worker that sentence without checking it. The test directories hold no fixture code files, and the marker-shaped text in the test sources is inside string literals, which hold no marker at the grammar level (`ScannedFile::scan_syntax`, `crates/mara-engine/src/code.rs`; DES-CODE-MARKERS in docs/code/index.mara.md). The orchestrator checked it on a copy of the repository at `4498546` with `include = ["crates/**/*.rs"]`: `project validate` reports 0 errors and the one known warning, with 219 code scans; in a debug build it takes 3.3 s instead of 2.0 s.

Decided by the maintainer on 2026-11-21, asked by the orchestrator to choose between keeping the two `src` patterns with a corrected comment and one pattern for everything: "Glob everything".

Done when `cargo run --package mara --locked --quiet -- --no-daemon project validate` reports 0 errors and no warning but the known `requirement-verified` one, `query execute implemented --limit 100` lists the same 94 links on 43 requirements, and the seven local checks pass.

- [x] `[code.languages.rust]` includes `crates/**/*.rs` and nothing else, and the comment above it says what is true: every Rust file of the crates is read at the grammar level, so only comments hold markers
- [x] The line on markers in AGENTS.md names the same scope

## Summary of Changes

- `.mara/config.toml`: `[code.languages.rust]` includes `crates/**/*.rs` only; its comment says every Rust file of the crates, tests included, is read at the grammar level, so only comments hold markers. The wrong sentence about fixture files is gone.
- AGENTS.md: the markers line names every `.rs` file under `crates/`.
- `project validate --debug-stats`: 0 errors, the known `requirement-verified` warning (docs/distribution.mara.md:7) only, 219 code scans (= `.rs` files under `crates/`), no marker or grammar diagnostic.
- `query execute implemented --limit 100`: identical to before, 94 links on 43 requirements, none under `tests/`.
- `project validate` time (hyperfine, 3 runs): debug 1.99 s → 3.28 s, release 309 ms → 502 ms.
- The seven local checks pass.

## Decisions made

- None beyond the maintainer's "Glob everything" recorded in the issue.
