---
title: "What merges no longer check since the release runs only by dispatch: the ubuntu-22.04 images and the packing"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-a3aq0n
created_at: 2026-11-23T11:16:47Z
---

Goal: decide whether anything of the release's checks should run before a release is dispatched, and build only that.

Context: decided by the maintainer on 2026-11-23: the release workflow starts only by a manual dispatch, as nx-rust's ("Can we have it manual as in nx-rust? and come back with enhancements later?"); a dry run on every push and on pull requests that change release inputs cost about 30 minutes on four runners per run, queued per ref (`ADR-DISPATCHED-RELEASE`). Since then a merge runs CI only, on `ubuntu-24.04`. What only the release's dry run exercises: `pnpm run checks` on `ubuntu-22.04` and `ubuntu-22.04-arm` (kernel 6.8, 1 ms file timestamps, slower: two test defects showed only there, mara-efyg6p and mara-4xsamc), the release build, the packing, the install verification and the `GLIBC_` floor. A break of any of these now shows at the dry dispatch before a release.

- [ ] The maintainer chooses: nothing more (the dry dispatch is enough); a `ubuntu-22.04` job in CI; or a dry run on pull requests that change release inputs only
