---
title: Decide whether a root rustfmt.toml should shield fmt-check from per-user Rustfmt configuration
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-788vbe
created_at: 2026-11-22T17:48:13Z
---

Goal: `fmt-check` gives the same verdict on every machine, and its cache key covers the configuration it reads.

Context: nx-rust 1.0.0-rc.2 keys no Rustfmt configuration outside the repository (`~/.rustfmt.toml`, `$XDG_CONFIG_HOME/rustfmt/`, a folder above the checkout); `fmt-check` then starts with a warning and a cached result can survive a change to that file. Its `docs/caching.mara.md`, "Rustfmt configuration outside the repository", names the answer: a `rustfmt.toml` at the repository root, even an empty one, makes rustfmt ignore the per-user files and is hashed. ADR-NX-RUST-TASKS states the limit. No such file exists on the maintainer's machine, so no warning appears there today. Adding the file changes formatting for a developer who has a per-user configuration, so it was left out of mara-788vbe.

- [ ] The maintainer decides; if yes, an empty root `rustfmt.toml` is committed and ADR-NX-RUST-TASKS says so
