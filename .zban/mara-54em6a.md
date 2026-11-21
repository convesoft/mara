---
title: A catalog URL that is no valid URL reports io-error, not invalid-argument
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-yk8drm
created_at: 2026-11-21T17:48:41Z
---

Goal: `mara code add` reports a catalog entry whose URL it refuses with the code the design names.

Context: P3 finding of the review of mara-yk8drm (Codex, at `560549d`, 2026-11-21; log `~/.tmp/codex/review-mara-yk8drm-560549d/findings.md`), there before that change. DES-GRAMMAR-ASSETS (docs/code/index.mara.md, near line 133) names `invalid-argument`; the mapping in `crates/mara/src/cli/code.rs` (near line 168) gives `io-error`. Exotic: only a malformed shipped catalog or the debug-only catalog override reaches it, and the failure comes before any connection.

- [ ] The code and DES-GRAMMAR-ASSETS agree on the diagnostic for an invalid catalog URL, with a test
