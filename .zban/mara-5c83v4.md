---
title: "Nx test key: tools and Git settings the tests use beyond the pinned corpus and Soufflé"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-a4ga3m
created_at: 2026-11-22T00:33:01Z
---

Goal: decide whether the cache key of the Nx target `mara:test` should follow the tools and Git settings the tests use besides the pinned Soufflé and the tree of `v0.3.0`.

Context: found by the review of mara-a4ga3m at `6ee187d` (P3, exotic on this project's machines; findings log `~/.tmp/codex/review-mara-a4ga3m-6ee187d/findings.md`). The tests start `git` with the ambient `PATH` (`crates/mara/tests/inputs.rs:685`), `mkfifo` (`:917`, `:960`), `git` and `tar` (`crates/mara/tests/convert/main.rs:96`, `:107`) and `cc` (`crates/mara/tests/programs/reach.rs:41`, `crates/mara/src/rules/sandbox/tests.rs:410`). The key holds neither `PATH` nor the identity of these tools, so a pass recorded with them is restored in an environment that lacks one, where `cargo test` would fail. Local Git archive attributes (`info/attributes`) can also change the archived corpus of the conversion tests without changing the tree the key hashes.

- [ ] Either the key follows what is worth following, or ADR-NX-RUST-TASKS says in one sentence that the tools on `PATH` are not part of it
- [ ] The summary says what was left out and why
