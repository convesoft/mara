---
title: "nextest configuration that the test key leaves out: an ignored .config/nextest.toml and the user configuration"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-rw6rhp
created_at: 2026-11-23T06:49:44Z
---

Goal: a change to any nextest configuration that a `test` task reads changes that task's Nx key, so that no result is restored for a run that the configuration would change. Reported to the maintainer of nx-rust; nothing in this repository works around it.

Context: with `testRunner = "nextest"`, nx-rust 1.0.0-rc.2 keys `test` with the output of `cargo nextest --version`, `.config/nextest.toml` and every `NEXTEST_*` variable (`ADR-NX-RUST-TASKS`; the plugin's `docs/caching.mara.md`). Found in the review of mara-rw6rhp:

- `.config/nextest.toml` is a plain Nx file input, which leaves out a file that Git ignores, while nextest still reads it. A developer whose ignored local configuration changes `profile.default.default-filter` can store a pass that is restored for a full run.
- nextest reads its user configuration (the file `NEXTEST_USER_CONFIG_FILE` names, or the per-user default) before the tests run and fails on an invalid one; the key holds the variable's value, not the file's contents.

This repository has no `.config/nextest.toml` and ignores none, so neither affects its checks today.

The fix belongs in the plugin's input declaration, `node_modules/@convesoft/nx-rust/src/graph/inputs.js` of 1.0.0-rc.2 (read-only here), for example a runtime input that hashes both files where they exist.

- [ ] Reported to the maintainer of nx-rust with both cases
- [ ] When a fixed release is pinned, `ADR-NX-RUST-TASKS` no longer names the two limits
