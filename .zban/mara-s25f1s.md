---
title: The docs-checks tests share their file and example helpers
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: one `read` and one `documented_example` in `crates/docs-checks`.

Context: architecture audit after rc.1. `crates/docs-checks/src/lib.rs` is an empty stub; `tests/examples.rs` and `tests/template.rs` each define `read` and a `documented_example` that differ: the first stops at the next `<a id=` anchor, the second does not, so a TOML block removed from `DES-PROJECT-CONFIGURATION` would silently be replaced by the next item's.

- [ ] The helpers live in the crate's library, with the bounded search, and all three test files use them
