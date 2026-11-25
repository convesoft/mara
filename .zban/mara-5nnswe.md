---
title: Gate native-only operations (convert, cli/init, templates, caller stubs) out of mara.wasm
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-8zm9hk
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #6, wave: `behavior`).

`crates/mara/src/cli.rs:95-120` compiles `init::PROJECT` and `convert::PROJECT_CONVERT` into `wasm32`, pulling ~2,100 lines of `convert/`, `cli/init.rs`, embedded templates, `granit-parser`, and `toml` `display`/`serde` features into `mara.wasm` and its `THIRD-PARTY-LICENSES`, even though `workspace.rs` unconditionally refuses both. `caller.rs:206-235` also carries `wasm32` no-op stubs only because native helpers in `rules.rs:42-79, 330-336` are compiled unconditionally.

- [ ] Gate `convert`, `cli::init`, `cli::convert`, `caller`, native `rules.rs` helpers, and native-only crate dependencies under `#[cfg(not(target_arch = "wasm32"))]`
