---
title: One reader of the workspace's cargo metadata for the packer and the release workflow
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

Goal: the version, the package list and the artifact directory are read from `cargo metadata` in one place, with one set of flags and one validation.

Context: architecture audit after rc.1. `release.yml` (Prepare) runs `cargo metadata --no-deps --offline --locked | jq` and checks SemVer with a shell pattern, then starts Node to import `packages/mara-npm/scripts/packages.mjs`, whose `workspace()` reads the same metadata without `--locked` and checks only for build metadata. `pack.mjs` calls `workspace()` at module load, before it has parsed its arguments, and `third-party-licenses.mjs` runs `cargo metadata` again with its own lookup of the workspace members. So `pack-dispatcher` accepts a stale `Cargo.lock` that `pack-native` and the workflow refuse.

- [ ] `workspace()` is the one reader (`--locked`, the version rule once), called after argument parsing; Prepare takes the version and the packages from one script call

Audit note (2026-11-25, post-mara-xzfeyj at `4cbcf26`, Finding #10): `release.yml`'s metadata read moved to `.github/scripts/release-prepare.sh:19-20`; `packages/mara-browser/copy-module.mjs:15-39` added another `cargo metadata --no-deps --locked` reader (using `process.cwd()`) and duplicated `pack.mjs`'s `CARGO_TARGET_DIR` guard, while `packages.mjs:workspace()` still lacks `--locked`. Unify into `requireWorkspaceTarget` in `packages.mjs`.
