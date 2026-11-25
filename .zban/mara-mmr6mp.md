---
title: Avoid cloning unchanged files in browser Workspace::new and Workspace::export
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #4, wave: `behavior`).

- `crates/mara/src/cli/workspace.rs:212` calls `memory::files()`, cloning every file's `Vec<u8>` in the workspace even when 0 or 1 files changed.
- `Workspace::new` (`workspace.rs:106-109`) clones every supplied file into `memory::reset`, which calls `.to_vec()` a second time (`system/memory.rs:387`), peaking at 3× memory inside the 32-bit WASM heap.

- [ ] Store `Arc<[u8]>` (or owned `Vec<u8>` without re-cloning) in `system::memory` and `Workspace.supplied`, and compare in place in `Workspace::export` so only added/modified files are cloned
