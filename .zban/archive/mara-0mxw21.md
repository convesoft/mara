---
title: "0.4.0-alpha.4: daemon"
status: done
type: epic
blocked_by:
  - mara-17tdb3
  - mara-bx63yb
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-21T16:12:31Z
---

Slice outcome: CLI and MCP calls are served warm by an on-demand per-user daemon that watches declared inputs.

Done when the daemon rows of VER-WORK-COUNTERS pass.

## Summary of Changes

Closed by the orchestrator on 2026-11-21 at `5607992`, when its eleven issues were closed: mara-17gykj, mara-jkabqy, mara-xqcnqw, mara-zmp8yz, mara-v9tfhw, mara-mg711b, mara-8hw038, mara-wz40n6, mara-p27z9t, mara-ef6ybb and mara-6za81b.

Done when: the daemon rows of VER-WORK-COUNTERS pass. The orchestrator ran them at the tree of `5607992` (the gate run of mara-6za81b at `7b5d9e2`, the same tree): `cargo test --workspace --locked` with the warm rows on the 1× corpus (`crates/mara/tests/performance/warm.rs`, `code_get.rs`) and the daemon tests (`crates/mara/tests/daemon/`), and `cargo test --package mara --test performance --release --locked` with the daemon scaling cases, all passing; no daemon row is skipped (only the nightly 64× counts are ignored). The other six local checks passed in the same run.

The decisions of the slice are in mara-b8cpdm. Left for later, each as its own issue: the backlog issues filed from this slice's reviews, and the contents of the shipped grammar catalog (mara-yk8drm, alpha.5).
