---
title: CI fails when the grammar download hits a network error
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-1avrq4
created_at: 2026-11-23T10:11:45Z
---

Goal: a transient network error while CI fetches the pinned Rust grammar does not fail the job.

Context: `DES-CI-WORKFLOW` and `ADR-CI-CHECKS` (docs/distribution.mara.md, docs/decisions.mara.md) fetch the grammar with `mara code add rust` on every run, with no retry and no cache. Run https://github.com/convesoft/mara/actions/runs/45135113220 on `7ca38c5` failed on `ubuntu-24.04-arm` in that step with `error[fetch-failed]: cannot fetch https://github.com/tree-sitter/tree-sitter-rust/releases/download/v0.24.2/tree-sitter-rust.wasm: io: failed to lookup address info`; the x64 job of the same run and the runs before and after passed. The release workflow's Checks jobs run the same step.

- [ ] The step survives a transient failure: a bounded retry in the workflow, or the grammar kept in the CI cache under a key of its pin (`DES-GRAMMAR-ASSETS`: the checksum is still verified), decided in `ADR-CI-CHECKS`
