---
title: "Split crates/mara-engine/src/evaluation/explain.rs: leaves and vocabulary"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-j4preg
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:21Z
---

Goal: `evaluation/explain.rs` holds the provenance commands, `Explainer`, `Node` and `Raw`; fact placement and printed-rule reading live in modules of their own.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: split; after mara-j4preg, which changes `Leaves` and `Node::write_json`. The file is about 1,530 lines. Suggested: `evaluation/explain/leaves.rs` (`Leaves`, placing of facts), `evaluation/explain/vocabulary.rs` (`Vocabulary`, reading printed rules).

- [ ] The two modules exist; no behaviour or result changes; the engine still builds for `wasm32`
