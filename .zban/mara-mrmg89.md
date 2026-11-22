---
title: Use the published @convesoft/nx-rust instead of the local checkout
status: in-progress
type: task
parent: mara-1fab1q
blocked_by:
  - mara-tt31bp
created_at: 2026-11-21T23:33:44Z
---

Goal: the repository gets `@convesoft/nx-rust` from npm at a pinned version, and nothing in it names a path on the maintainer's machine.

Context: the epic mara-1fab1q. Until the package is published, mara-a4ga3m loads the plugin from the TypeScript source of the maintainer's local checkout: `nx.json` names the entry file by an absolute path, `package.json` has a `link:` dependency on the source folder, and every `nx` command needs `NX_PREFER_NODE_STRIP_TYPES=false`. That works on one machine only, and a cache does not notice a change of the plugin's source. Asked by the maintainer on 2026-11-22: "just add additional zban issue to switch to the published version at the end of this new epic"; the maintainer expects the package to be published at about the time the setup is finished. This issue waits for that release: if the package is not on npm when its turn comes, it stays open and the orchestrator asks the maintainer whether the epic closes without it.

- [ ] `package.json` and the lockfile have `@convesoft/nx-rust` from the npm registry at an exact version, and `nx.json` names the plugin by its package name, as its `init` generator writes it
- [ ] What only the local source needed is gone where nothing else uses it: the `link:` dependency, the absolute path, `NX_PREFER_NODE_STRIP_TYPES=false` in AGENTS.md and the scripts, and the transpiler packages and `tsconfig.base.json` if no project of the repository needs them
- [ ] The Decision item that records the interim dependency says what replaced it
- [ ] Every local check passes through the `package.json` script in a fresh worktree after `pnpm install`, and the cache keys follow the plugin's version (nx-rust puts its version into them)

## Folded into mara-a4ga3m (the maintainer, 2026-11-22)

From its TypeScript source the plugin runs no `test` and caches no `build` (mara-tt31bp), and the maintainer chose to wait for the npm release. The worker of mara-a4ga3m does this issue's items in its own change and closes this issue with it. No separate worker starts it.
