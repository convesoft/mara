---
title: CI and the release run their checks through one shared action, the clean-tree check included
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: the steps that fetch the grammar, run `pnpm run checks` and verify the tree are written once.

Context: architecture audit after rc.1. `ci.yml` and the Checks job of `release.yml` each write the full-history checkout, `mara code add rust` and `pnpm run checks`. They already differ: CI checks afterwards that no check changed the tree, the release does not. The grammar download's hardening (the backlog issue on its network failure) would have to be made twice. Node's version is pinned in four places.

- [ ] One composite action for the grammar, the checks and the clean-tree check, used by both workflows; the release's Checks verify the tree
