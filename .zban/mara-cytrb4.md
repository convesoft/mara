---
title: A write or a get of code in a session copies no inventory it does not change
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-agdnwh
created_at: 2026-11-21T15:20:11Z
---

Goal: no request in a daemon session copies the session's inventory (directories, `.gitignore` files, parsed documents, programs) unless it changes it.

Context: found while fixing mara-agdnwh, which split `State` in `crates/mara/src/inputs.rs` into a shared `Inventory` and the session's code maps, and lets a view release its inventory before `Inputs::generation` derives on the session. Two places still copy it:

- `Inputs::written` (the write hand-over of DES-DAEMON-SESSIONS) runs `generation_for(Derivation::Written)` on the session while the request's view still holds the inventory it adopted, so `Arc::unwrap_or_clone` copies the inventory on every write served in a session. `Inputs::apply` takes `&self`, so the view cannot let go of it there without a signature change.
- `Inputs::code_file` (`get` of a code location) clones the inventory's `directories` and `gitignores` maps on every call to seed its listing walk.

Each copy is O(inputs) clones of paths and `Arc`s, beside the O(inputs) stats the same request makes; no work counter counts it.

- [ ] A write served in a session does not copy the inventory
- [ ] `get` of a code location lists without cloning the inventory's maps
