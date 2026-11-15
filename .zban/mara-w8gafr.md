---
title: Read-back guard accepts an inserted item that ends before its text
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
related:
  - mara-sjjsnm
discovered_from: mara-w0xasr
created_at: 2026-11-15T15:22:49Z
---

Goal: `Expect::Item` guarantees what its doc comment says, "an item text that reads back as one item of exactly those bytes", or the comment and DES-ITEM-WRITER say that an inserted item may end early.

Context: `crates/mara-engine/src/write/read_back.rs`, `Reading::inserted`, refuses an inserted item whose span runs past its text but accepts one that ends before it. A thematic break or a MID anchor in the inserted text ends the item there; the rest reads as narrative or another item inside the written bytes, which the guard also allows. DES-ITEM-WRITER says "ends within its text", so the guard follows the doc but not the `Expect::Item` comment.

Smallest case: `mara_engine::write::insert("", 0, &item)` where `item` is `write::item(mid, 2, "T", &[row("type","Requirement")], "A.\n\n---\n\nB.", "\n")`; `ReadBack::edit` accepts it.

mara-w0xasr guards against this in the operations (`authoring::own_item`: no other item or narrative unit may start inside the written item or body), so `item create` and `item update` refuse such bodies with `unreadable-edit`. Nothing user-visible is open.

- [ ] Decide whether the guard itself should refuse an inserted item that ends before its text, and align DES-ITEM-WRITER and the `Expect::Item` comment
