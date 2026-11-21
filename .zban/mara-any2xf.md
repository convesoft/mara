---
title: Direct-mode writes copy no buffer for a hand-over that does not happen
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-5mafg9
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: a write in direct mode moves its buffers; parses and copies made for a session's hand-over exist only where a session takes them.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: behavior; do it with or after mara-5mafg9 (one write path for relink and fix). `crates/mara/src/cli/fix.rs` (near line 72), `cli/relink.rs` (near 192) and `cli/author.rs` (near 378) copy `new.contents.to_vec()` into `FileWrite` and keep the `ParsedFile` for `Inputs::apply`; in direct mode `Inputs::written` drops it at once. `relink.rs` also clones every written `Document` into a vector that `item_rename` discards and of which `item_move` reads one line number. Backlog mara-cytrb4 holds the copy of the inventory in the hand-over itself.

- [ ] Direct mode copies no write buffer and keeps no parse; `item_move` takes the one line it needs
- [ ] Unchanged results and work counts
