---
title: "Decide how nx-rust is loaded until it is on npm: from its source it runs no test and caches no build"
status: todo
type: task
tags:
  - maintainer
discovered_from: mara-a4ga3m
created_at: 2026-11-22T00:18:19Z
---

Goal: the maintainer chooses how this repository gets nx-rust until the package is on npm, so that mara-a4ga3m can finish: its `test` target must run and its `build` target must be cached.

Context: the maintainer decided on 2026-11-22 that the plugin loads from the TypeScript source of the local checkout, without a build (mara-c4y5ts). The worker of mara-a4ga3m set that up (branch `chore/init-nx-workspace`, head `6ee187d`, not integrated) and found two gaps, which the orchestrator confirmed by reading the plugin at `0078ee2`:

- `nx-rust/src/executors/runner-script.ts:9` returns `join(__dirname, "test-runner.js")`. Each test binary is wrapped in `node …/src/executors/test-runner.js`; a source checkout has only `test-runner.ts`, so every `test` task fails with `Cannot find module`.
- `nx-rust/src/graph/inputs.ts:834` names `../cargo/print-artifact-folders.js` as a command input of `build`. It fails the same way, the plugin then puts a random value into the key on purpose, and `build` is never restored.

`fmt-check`, `check` and `lint` pass through Nx from source, are cached and run again when an input changes. The plugin checkout is read-only for this project's agents and no agent builds in it, so each option needs the maintainer or the nx-rust project:

1. Link to a build of the plugin: someone builds it in the nx-rust checkout, and this repository's `link:` dependency points at the build output, with the plugin named by its package name in `nx.json`. The transpiler-only needs (`NX_PREFER_NODE_STRIP_TYPES=false`, the absolute path of `src/index.ts`) go away, and the layout is the one the npm package has, so mara-mrmg89 becomes a change of one dependency. The build is a snapshot: it does not follow the source until it is rebuilt. Recommended by the orchestrator.
2. Fix nx-rust so that it finds and runs its two helper scripts when it is loaded from source. Keeps the decision as it is; it adds code to the plugin for a mode the published package never uses. Recommended by the worker.
3. Wait for the npm release: mara-a4ga3m stays parked and takes the package from npm, which does mara-mrmg89 in the same change. The registry holds only the bootstrap placeholder `0.0.0` on 2026-11-22.

Two more findings about the plugin, for the maintainer to take upstream (nothing here waits for them):

- `build.warnings = "deny"` does nothing under nx-rust: Cargo 1.100 ignores it (and `CARGO_BUILD_WARNINGS`) with `--message-format=json`, which the plugin always passes (plain cargo: exit 101 without the flag, exit 0 with it). The plugin's docs recommend the setting and it still enters the cache key. This repository denies warnings with `RUSTFLAGS=-D warnings` in `.env.lint` instead.
- From source, every executor run prints `Convert compiler options from json failed, No inputs were found in config file 'tsconfig.json'` (`@swc-node/register` 1.11.1 reading the preset's `tsconfig.json` with `"files": []`). Cosmetic; gone with a built package.

- [ ] The maintainer has chosen, and mara-a4ga3m is unblocked with the answer in its brief
