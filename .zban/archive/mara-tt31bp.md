---
title: "Wait for the npm release of @convesoft/nx-rust: mara-a4ga3m takes the plugin from npm"
status: done
type: task
tags:
  - maintainer
discovered_from: mara-a4ga3m
created_at: 2026-11-22T00:18:19Z
closed_at: 2026-11-22T00:33:08Z
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

- [x] The maintainer has chosen
- [x] A real version of `@convesoft/nx-rust` (not the `0.0.0` placeholder) is on npm, and the orchestrator has resumed mara-a4ga3m with it

## Decided by the maintainer on 2026-11-22

Option 3, "Wait for npm". mara-a4ga3m stays parked until a real version of `@convesoft/nx-rust` is published. Its worker then takes the package from npm at an exact version, names the plugin by its package name, drops what only the source needed (the `link:` dependency, the absolute path, `NX_PREFER_NODE_STRIP_TYPES=false`, and the transpiler packages and tsconfig files only if nothing else needs them), and closes mara-mrmg89 in the same change. This replaces the decision of 2026-11-22 to load the plugin from its TypeScript source. Nothing of the epic or of alpha.6 (mara-xzfeyj) starts until then. This issue stays open as the marker of that wait; the orchestrator closes it when the package is there.

## Summary of Changes

`@convesoft/nx-rust@1.0.0-rc.1` was published on 2026-11-22 at 00:26 UTC under the dist-tag `next` (391 files, built from `0078ee2`; `latest` is still the `0.0.0` placeholder). The orchestrator resumed mara-a4ga3m with it: the worker pins that exact version and does mara-mrmg89 in the same change. No code changed for this issue.

More findings about the plugin from the review of mara-a4ga3m at `6ee187d`, for the maintainer to take upstream (reproductions in `~/.tmp/codex/review-mara-a4ga3m-6ee187d/findings.md`); none blocks this repository, which uses none of these settings:

- `lint` hashes the value of `CLIPPY_CONF_DIR`, not the configuration file it selects: an edit of that file restores the old result.
- `fmt-check` does not follow a per-user Rustfmt configuration (`$XDG_CONFIG_HOME/rustfmt/rustfmt.toml`): an edit restores the old result.
- `check`, and the compilation of `test`, do not follow Cargo configuration outside the repository (`build.rustflags` in the Cargo home): an edit restores the old result. The plugin prints its warning about configuration outside the repository.
