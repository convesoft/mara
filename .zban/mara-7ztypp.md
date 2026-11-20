---
title: Review the decisions workers made in alpha.5
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-779tms
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.5 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.5 epic (mara-779tms) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf under the rule of 2026-11-16: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-16, mara-ttvp9z: inline marker comments stay supported (`check(); // @mara implements REQ-CHECK`), as in 0.3.0; the worker's own-line rule would have dropped them. Rests on REQ-CODE-MARKER-LINKS and AGENTS.md (0.3.0 where the capability is kept). Undo: not needed; dropping them would be the maintainer's decision.
- 2026-11-16, mara-ttvp9z: a missing grammar is the error `unavailable-grammar`, so `project validate` fails on a fresh machine until `mara code add` has run. Rests on DES-DIAGNOSTICS and REQ-ONLY-CODE-ADD-DOWNLOADS. Undo: allow a warning for this one case.
- 2026-11-16, mara-ttvp9z: a project with no code language fails the template's `verified` gate instead of reporting it unavailable. Rests on DES-CODE-READERS (code counts as completely read) and the gate's design. Undo: change the gate design.
- 2026-11-16, provisional, mara-ttvp9z: ADR-GRAMMAR-TRUST-ON-FIRST-USE (Draft): grammars in the built-in catalog are checked against built-in checksums; a repository the user names is trusted on first use, its fetched bytes and repository are pinned, and every later fetch and load is checked against the pin; transport is HTTPS, or HTTP only to the local machine. No Accepted item covered this before; it weakens none. A downloaded grammar is a `.wasm` file run inside tree-sitter's WebAssembly runtime in Mara's process, without access to files, the network or native code; no CPU or memory limit is set for it. Undo: require a checksum argument for every repository outside the catalog.
- 2026-11-16: mara-ttvp9z was started before the alpha.4 epic closed, and mara-yhxq5x was split (mara-hk2arx for programs and the template's code cases, mara-9fphtg for the daemon) and started beside alpha.3 and alpha.4. Undo: not needed.

- 2026-11-16, confirmed by the maintainer: asked how Mara should trust a grammar from a repository the user names that is not in the catalog (options: trust on first use; require a checksum; catalog only), the maintainer chose "Trust on first use (Recommended)". ADR-GRAMMAR-TRUST-ON-FIRST-USE is no longer provisional; mara-h0hxa3 sets it to Accepted.

- Orchestrator, 2026-11-20 (mara-s9gak4): a pinned `.wasm` that is valid WebAssembly but no tree-sitter grammar aborts the process inside the tree-sitter runtime. Rated exotic (only a pin whose checksum names such a file reaches it; ADR-GRAMMAR-TRUST-ON-FIRST-USE): the worker fixes it if small, otherwise states the limit in DES-GRAMMAR-ASSETS and files a backlog issue. Undo: do that backlog issue.
- Orchestrator, 2026-11-20: mara-hk2arx is started before mara-jm5ecb and mara-s9gak4 are integrated; it rebases over both.
