---
title: "0.4.0-alpha.1: read and validate documents from the CLI"
status: done
type: epic
blocked_by:
  - mara-km8qqj
created_at: 2026-11-13T22:08:55Z
closed_at: 2026-11-14T22:42:22Z
---

Slice outcome: `mara` parses the v0.4.0 item format, validates a project and answers read operations in direct mode.

Done when this repository's `docs/` validates with the built binary and the work-counter and scaling tests pass locally.

## Summary of Changes

Closed by the orchestrator on 2026-11-14 at integration commit 5819707. All children are closed. Done when, run on the integration checkout at that commit with a clean tree: `cargo run --package mara --locked --quiet -- project validate` reports `valid: 0 errors, 0 warnings` for this repository's `docs/` (30 documents, 171 items, 328 edges; 33 file reads, 33 hashes, 30 parses, 1 snapshot build); the work-counter and scaling tests pass (`cargo test --workspace --locked`: 247 passed; `cargo test --package mara --test performance --release --locked`: 9 passed). The other local checks pass too: fmt, clippy with warnings denied, the wasm32 engine build, `zban check`. The workers' decisions are appended to mara-5ynggq.
