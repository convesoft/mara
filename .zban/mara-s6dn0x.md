---
title: The local checks do not check the formatting of JS files
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-svq540
created_at: 2026-11-22T21:39:15Z
---

Goal: `pnpm run checks` fails when a JS file under `packages/` is not formatted by oxfmt.

Context: oxfmt is configured (`.oxfmtrc.json`), but no Nx target runs it: `checks` runs `fmt-check` (inferred by nx-rust for crates only), `lint` (oxlint, inferred by `@nx/oxlint` for `packages/mara-npm` too), `test`, `validate` and `zban-check`. mara-svq540 added the first JS files (`packages/mara-npm`) and formatted them by hand with `pnpm exec oxfmt`. Adding a check needs the maintainer's approval (AGENTS.md, "Engineering") and an entry in ADR-NX-RUST-TASKS's table.

- [ ] The maintainer decides whether JS formatting is a local check, and through which target
- [ ] If so, the target exists, is cached with its inputs declared, and `checks` runs it
