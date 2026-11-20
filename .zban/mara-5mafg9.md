---
title: One Markdown link parser, and one write path for relink and fix
status: todo
type: task
parent: mara-qzp1gd
created_at: 2026-11-20T20:22:51Z
---

Goal: links are found by one parser, and the operations that rewrite links write through one path with one read-back rule.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: DES-WRITE-PROCEDURE (docs/authoring/writes.mara.md) and the items for `project fix` and for `item move` / `item rename` in docs/authoring/. Data safety: every write stays checked before it replaces a file (REQ-INTERRUPTED-EDIT-LOSES-NOTHING); this issue changes structure, not behaviour, and any difference in what the two parsers accept today is a finding to report, not to decide silently.

- [ ] `crates/mara-engine/src/fix.rs` and `crates/mara-engine/src/relink/links.rs` each parse Markdown links and can drift apart. One shared parser; list every input on which the two disagree today and say which result is kept and why
- [ ] `crates/mara-engine/src/relink.rs`, `crates/mara/src/cli/author.rs` and `crates/mara/src/cli/fix.rs`: `relink` and `fix` handle the read-back differently and repeat the CLI's write-session scaffolding. One write-session path with one read-back rule

Done when both are done, the existing tests for fix, move and rename pass unchanged or their changes are listed with the reason, and the local checks pass.
