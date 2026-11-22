---
title: Use the published @convesoft/nx-rust instead of the local checkout
status: done
type: task
parent: mara-1fab1q
blocked_by:
  - mara-tt31bp
created_at: 2026-11-21T23:33:44Z
closed_at: 2026-11-22T01:05:54Z
---

Goal: the repository gets `@convesoft/nx-rust` from npm at a pinned version, and nothing in it names a path on the maintainer's machine.

Context: the epic mara-1fab1q. Until the package is published, mara-a4ga3m loads the plugin from the TypeScript source of the maintainer's local checkout: `nx.json` names the entry file by an absolute path, `package.json` has a `link:` dependency on the source folder, and every `nx` command needs `NX_PREFER_NODE_STRIP_TYPES=false`. That works on one machine only, and a cache does not notice a change of the plugin's source. Asked by the maintainer on 2026-11-22: "just add additional zban issue to switch to the published version at the end of this new epic"; the maintainer expects the package to be published at about the time the setup is finished. This issue waits for that release: if the package is not on npm when its turn comes, it stays open and the orchestrator asks the maintainer whether the epic closes without it.

- [x] `package.json` and the lockfile have `@convesoft/nx-rust` from the npm registry at an exact version, and `nx.json` names the plugin by its package name, as its `init` generator writes it
- [x] What only the local source needed is gone where nothing else uses it: the `link:` dependency, the absolute path, `NX_PREFER_NODE_STRIP_TYPES=false` in AGENTS.md and the scripts, and the transpiler packages and `tsconfig.base.json` if no project of the repository needs them
- [x] The Decision item that records the interim dependency says what replaced it
- [x] Every local check passes in a fresh worktree after `pnpm install` (through the `package.json` script once mara-jmt2hr adds it), and the cache keys follow the plugin's version (nx-rust puts its version into them)

## Folded into mara-a4ga3m (the maintainer, 2026-11-22)

From its TypeScript source the plugin runs no `test` and caches no `build` (mara-tt31bp), and the maintainer chose to wait for the npm release. The worker of mara-a4ga3m does this issue's items in its own change and closes this issue with it. No separate worker starts it.

## Summary of Changes

Done in the change of mara-a4ga3m (branch `chore/init-nx-workspace`).

- `package.json` pins `@convesoft/nx-rust` at `1.0.0-rc.1` from the npm registry (dist-tag `next`, a release candidate). `pnpm-lock.yaml` resolves it from the registry with its integrity hash and holds no path of this machine. `nx.json` names the plugin `@convesoft/nx-rust` with `"options": {"locked": true}`, plus the five `targetDefaults` entries that the plugin's `init` generator wrote in a temporary workspace under `$TMPDIR`.
- pnpm 12 refuses the release while it is younger than pnpm's minimum release age (`ERR_PNPM_MINIMUM_RELEASE_AGE_VIOLATION` from `pnpm install --frozen-lockfile`), so `pnpm-workspace.yaml` has `minimumReleaseAgeExclude: ["@convesoft/nx-rust@1.0.0-rc.1"]`. pnpm wrote that entry, and it names this one version only.
- Removed: the `link:` dependency, the absolute path in `nx.json`, and `NX_PREFER_NODE_STRIP_TYPES=false` from AGENTS.md (no script had it). The preset's `tsconfig.base.json`, `tsconfig.json`, transpiler packages and `packages/` stay for the alpha.6 packages.
- ADR-NX-RUST-TASKS says the plugin comes from npm at that pinned version, with loading from source, linking to a build and changing the plugin as rejected alternatives.
- The keys follow the plugin's version: every Rust target lists the input `{"externalDependencies": ["@convesoft/nx-rust"]}` (`pnpm nx show target mara:test --json`). Its graph node, in `.nx/workspace-data/project-graph.json` under `externalNodes["npm:@convesoft/nx-rust"]`, is `{"version": "1.0.0-rc.1", "hash": "sha512-fgwiq/…"}`, so a new version or tarball changes every key.
- Checks: `pnpm install --frozen-lockfile` passes in a fresh copy at another depth. The seven local checks pass, and `fmt-check`, `lint`, `check`, `build` and `test` pass through Nx (details in mara-a4ga3m).

## Decisions made

- Exempt exactly `@convesoft/nx-rust@1.0.0-rc.1` from pnpm's minimum release age, as pnpm itself wrote it. The exemption can go once the release is older than the limit.
