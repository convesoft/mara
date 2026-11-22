---
title: "Initialize the Nx workspace with nx-rust: the crates are projects and their inferred targets pass"
status: done
type: feature
parent: mara-1fab1q
blocked_by:
  - mara-tt31bp
created_at: 2026-11-21T23:23:19Z
closed_at: 2026-11-22T01:06:20Z
---

Goal: after `pnpm install` in a checkout, `nx show projects` lists `mara` and `mara-engine` and the workspace project, and the targets nx-rust infers for them (`fmt-check`, `lint`, `check`, `build`, `test`) pass, are cached, and run again when something they read changes.

Context: the epic mara-1fab1q, with the maintainer's decisions. AGENTS.md ("Engineering") lets no build system in that `docs/` does not specify unless the maintainer approves it; the maintainer asked for this one, and this issue writes the decision into the docs. nx-rust's own documentation is in the maintainer's checkout, `/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust/docs/` (`setup.mara.md`, `targets.mara.md`, `caching.mara.md`, `configuration.mara.md`, `testing.mara.md`): read-only for this work. The package is not on npm, and it needs no build: Nx loads the plugin from its TypeScript source in the maintainer's checkout, `/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust`. The orchestrator tried it on 2026-11-22 in a throwaway Cargo workspace with Nx 24.0.6 and Node 26: `nx show projects` listed the crate and the workspace project, and an inferred target ran and was cached, with (1) `nx.json` naming the plugin by its entry file, `/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust/src/index.ts` (by package name it fails: the package's `main` is `./src/index.js`, which exists only in a build); (2) the dev-dependency `"@convesoft/nx-rust": "link:/home/araketski/Work/github.com/convesoft/nx-rust/nx-rust"`, so that the executors (`@convesoft/nx-rust:cargo` and the others) resolve; (3) `@swc-node/register`, `@swc/core` and `typescript` installed and a `tsconfig.base.json` in the workspace root; (4) `NX_PREFER_NODE_STRIP_TYPES=false` in the environment of every `nx` command, because Node's own type stripping refuses the plugin's `export =`. The plugin then runs as that checkout is at the moment, whatever branch and uncommitted changes it holds, and its version string does not change when its source does.

What the plugin cannot see and the cache keys must still cover (found by the orchestrator at `46ccb4a`; check for more):
- Files outside the crates that their code and tests read: `docs/format.mara.md` and `docs/rules/index.mara.md` by `include_str!` in `crates/mara-engine/src`, and `crates/mara/tests/documents.rs` reads `docs/` whole (`fileInputs`).
- The Soufflé binary the tests run, `target/souffle-2.5/souffle`, built by `scripts/build-souffle.sh`: a file pattern may not point into Cargo's artifact directory, so it is a command input (`runtimeInputs`) or Soufflé moves out of `target/`.
- Environment variables that change what the tested client does: `CI` and `MARA_NO_DAEMON` (`crates/mara/src/cli/client.rs`); look for others (`envInputs`).
- The pinned Rust grammar in the per-user cache, for anything that validates this repository's own project.

- [x] The decision is an item in `docs/` (a Decision in docs/decisions.mara.md, or where the docs keep such choices): Nx with nx-rust runs the repository's tasks, cargo stays the build tool, the interim dependency by path and what replaces it, with the rejected alternatives one line each
- [x] Root files: a private `package.json` with a pinned package manager and Nx 24, `pnpm-lock.yaml`, `nx.json` with the plugin registered as its `init` generator does, and `.gitignore` entries for `node_modules` and Nx's own folders; nothing is written into the crates but `[package.metadata.nx]` or `[workspace.metadata.nx]` tables
- [x] The inputs above are declared, and the summary shows for each that changing it runs the affected target again and that an unchanged tree restores from the cache
- [x] `fmt-check`, `lint` (with warnings denied, as the local check does), `build` and `test` (locked) pass through Nx and give the results the cargo commands give; the cargo commands still pass
- [x] AGENTS.md says what a fresh worktree needs (`pnpm install`) and how agents on this machine run Nx (no daemon, the local Nx), in the fewest lines; "Local checks" keeps its cargo commands until the next issue

How the workspace is created (the maintainer's way, 2026-11-22: "init some suitable preset (which contains files we will need in 0.6 and tsconfig) in temp folder and copy the files we need to make sure we aligned with what nx generates"): generate Nx's TypeScript preset in a temporary directory outside every git repository, and copy from it only the workspace files. The orchestrator ran it once on 2026-11-22 to see what it writes: `pnpm dlx create-nx-workspace@24.0.6 <name> --preset=ts --packageManager=pnpm --nxCloud=skip --skipGit --no-interactive` under `$TMPDIR` (Nx maps the preset to its template `nrwl/empty-template`).
- Copy, adapted to this repository: `package.json` (private; the repository's name and licence; `nx`, `@nx/js`, `@swc-node/register`, `@swc/core`, `@swc/helpers`, `@types/node`, `tslib`, `typescript`; the pinned package manager), `nx.json` (the named inputs and the `@nx/js/typescript` plugin with its `typecheck` and `build` targets), `pnpm-workspace.yaml` (`packages/*`), `tsconfig.base.json` and `tsconfig.json` (project references; the custom condition renamed from the template's `@org/source`), `packages/.gitkeep`, and the `.gitignore` entries for `node_modules`, build output and Nx's folders, merged into the repository's file. These are what the alpha.6 packages need, and the transpiler packages and the `tsconfig.base.json` are also what loading nx-rust from source needs.
- Agent files (the maintainer, 2026-11-22: "We might want some of them like skills and managed nx block in agents.md"). Take two things from what the generator writes for coding agents. (1) The managed block of its `AGENTS.md`, between `<!-- nx configuration start-->` and `<!-- nx configuration end-->`, 23 lines, unchanged, so that Nx can update it; `CLAUDE.md` is a symbolic link to `AGENTS.md` here, so one copy serves both. The file is 98 lines and its own rule allows 120: the size rule gets the words that the managed Nx block does not count (agreed by the maintainer on 2026-11-22). The block speaks of an Nx MCP server, which is not configured here: say so in one line outside the block rather than editing inside it. (2) The Nx skills of `.agents/skills/` that this repository will use: `nx-workspace`, `nx-run-tasks`, `nx-generate`, `nx-plugins` and `link-workspace-packages`; not `monitor-ci` (Nx Cloud is not used) and not `nx-import`. Keep one copy of them, where the agents used on this project (Claude, Codex, Gemini) find project skills, and say where that is and how you checked it.
- Do not take the rest: the MCP server entries (`.codex/config.toml`, `.gemini/settings.json`: each agent session would start `nx mcp`), the generated `.claude/settings.json` (it enables a plugin marketplace from GitHub and allows an analytics domain; this repository's own `.claude/settings.json` stays as it is), `.cursor/`, `.github/`, `.opencode/`, `opencode.json`, `.vscode/` and `README.md`.
- Linter and formatter: oxlint and oxfmt (the maintainer, 2026-11-22: "oxlint and oxfmt for sure"). The preset cannot select them: run with `--linter=oxlint --formatter=oxfmt` it still downloaded the template with Prettier and no linter. Adding them afterwards costs nothing, by the installed Nx 24.0.6 packages (the Nx website's Oxlint pages did not load when the orchestrator looked): `nx format` detects the formatter at every run (an oxfmt configuration file wins over Prettier's; without either, the dependency in the root `package.json` decides), and `@nx/oxlint` has an `init` generator that registers the plugin and a `configuration` generator per project. So leave out `prettier`, `.prettierrc` and `.prettierignore`, and add `oxfmt` with `.oxfmtrc.json`, and `oxlint` and `@nx/oxlint` (at the Nx version) with `.oxlintrc.json`, as the nx-rust repository has them. Take what `nx add @nx/oxlint` writes from a run in the temporary workspace, not from a run in this checkout. The formatter must leave the Mara pages (`docs/**`), `.mara/` and `.zban/` alone (ignore patterns): Mara writes its pages in its own layout. The oxlint plugin must not change the crates' Rust targets (nx-rust's `targets.mara.md`, "Other plugins that reach Rust targets"): check that `mara:lint` still runs clippy.
- The template pinned `nx` and `@nx/js` at 24.0.0 while nx-rust is tested with 24.0.6: use one exact 24.x version for both and say which. The preset was run with `--skipGit`; never run the generator, `nx init` or `nx add` inside this repository's checkout, because the generator behaves differently when it finds a coding agent and can run `git add` and `git commit --amend` (`createTestProject` in nx-rust's `nx-rust-e2e/src/nx-rust.spec.ts`), which AGENTS.md forbids. Then add nx-rust as described above, with the entries its `init` generator writes.

## What replaced loading from source (2026-11-22)

Loading from source, as the Context describes it, was built and then dropped: it runs no `test` and restores no `build`, because nx-rust starts `src/executors/test-runner.js` and `src/cargo/print-artifact-folders.js`, which exist only in a build (mara-tt31bp). The maintainer chose the npm release. The repository depends on `@convesoft/nx-rust` at exactly `1.0.0-rc.1`, and `nx.json` names it by package name. No `link:` dependency, absolute path or `NX_PREFER_NODE_STRIP_TYPES` remains. This change also does mara-mrmg89. ADR-NX-RUST-TASKS records the result.

## Summary of Changes

The repository is an Nx 24.0.6 workspace. `@convesoft/nx-rust@1.0.0-rc.1` comes from npm (this change also does mara-mrmg89) and makes `mara` and `mara-engine` projects, beside the workspace project `cargo-workspace`. ADR-NX-RUST-TASKS records the decisions.

- **Root files** from the TypeScript preset: `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `tsconfig.base.json`, `tsconfig.json`, `packages/.gitkeep`, `nx.json`, oxlint (`.oxlintrc.json`, from `nx add @nx/oxlint` in a temporary workspace) and oxfmt (`.oxfmtrc.json`).
- **Cache in the checkout:** `nx.json` sets `"cacheDirectory": ".nx/cache"`. Without it, Nx 24.0.6 shares one per-user folder between all checkouts of the repository.
- **Ignore rules** in `.gitignore` are anchored to the repository root, so nothing under `docs/` or `.mara/` is ignored by them.
- **Agent files:** the managed Nx block in AGENTS.md, and five Nx skills in `.agents/skills/`, with `.claude/skills` linking to it. The installed binaries show where each agent looks: Claude Code 2.1.330 loads project skills only from `.claude/skills`; Codex 0.171.0 and Antigravity read `.agents/skills`.
- **Inputs:** `[package.metadata.nx]` in both crates. `fileInputs` cover the docs pages read by `include_str!`. The `mara:test` inputs cover `docs/**`, `.mara/**`, `CI`, `MARA_NO_DAEMON`, `TMPDIR`, the `MARA_TEST_*` hooks, the mode and SHA-256 of `${CARGO_TARGET_DIR:-target}/souffle-2.5/souffle`, and the tree of `v0.3.0`.
- **Warnings:** `.env.lint` sets `CARGO_ENCODED_RUSTFLAGS=-Dwarnings`, so `lint` denies warnings whatever `RUSTFLAGS` the caller has.

Checks, at the final head, with `export NX_DAEMON=false NX_USE_LOCAL=true` after `pnpm install --frozen-lockfile` and `scripts/build-souffle.sh`:

- The seven local checks of AGENTS.md pass. `cargo test --workspace --locked`: 823 passed, 0 failed, 14 ignored, in 26 binaries. Performance: 32 passed.
- `berth -- pnpm nx run-many -t fmt-check lint check build test --output-style=static` passes from an empty `.nx/cache` (0/11 hits; `test` 823 passed, 0 failed, 14 ignored, as cargo gives). A second run restores all ten crate tasks, `build` included (10/11; the `lockfile` task is never cached).
- **Cache location:** entries appear in `.nx/cache`. `find ~/.nx/3940d6fc279a5a0b -newer <marker>` lists nothing after these runs; that is the folder Nx 24.0.6 would share for this repository.
- **Lint:** add `fn _probe() { let unused = 1; }` to `crates/mara-engine/src/lib.rs`. Then `pnpm nx lint mara-engine` exits 1 with `RUSTFLAGS` unset and with `RUSTFLAGS='-C debuginfo=1'`, twice each; after reverting, it exits 0 both ways.
- **Cache probes for `fmt-check`/`check`/`lint`.** Each change ran the affected tasks again, and reverting it restored them:

  | Change | Ran again |
  | --- | --- |
  | crate source | all six |
  | `docs/format.mara.md` | all six |
  | `docs/project.mara.md` | `mara` only |
  | `rust-toolchain.toml` | all six |
  | workspace `Cargo.toml` | all six |
  | `Cargo.lock` | `check`, `lint` |
  | `.env.lint` | `lint` |

- **Probes for `mara:test`** (`pnpm nx run-many -t test -p mara`). Each change ran it again and reverting restored it: `docs/index.mara.md`, read only by the tests; a newline appended to `.mara/schema.toml` (the test then fails); `MARA_NO_DAEMON=1`; `CI=probe2` (1/3 hits, then 2/3); `TMPDIR` set to another folder; `chmod a-x` on the Soufflé binary (the test fails); a byte appended to it. The tag `v0.3.0` was not touched.
- **Ignore rules:** `git check-ignore -v docs/tmp/x.mara.md docs/dist/x.mara.md .mara/tmp/x` prints nothing.

## Decisions made

- Inputs that only `mara`'s tests read are inputs of its `test` target, not `envInputs` or `runtimeInputs`, so `build`, `check` and `lint` do not run again for them. Files read by `include_str!` are `fileInputs`.
- No target validates the repository's own project, so the per-user grammar is no input (that check becomes a target in mara-jmt2hr).
- `lint` denies warnings through `CARGO_ENCODED_RUSTFLAGS=-Dwarnings` in `.env.lint`. Cargo ignores `build.warnings` under nx-rust's `--message-format=json`, and Nx does not set `.env.lint`'s `RUSTFLAGS` over a caller's value. The limit: during `lint` a caller's `RUSTFLAGS` is not applied, and a caller's `CARGO_ENCODED_RUSTFLAGS` replaces the denial.
- The Soufflé input follows `CARGO_TARGET_DIR`, not a target directory set only in Cargo configuration.
- `nx.json` sets `cacheDirectory`, so worktrees share no Nx results, as mara-c4y5ts requires.
- The plugin runs in locked mode.
- oxfmt skips `docs/`, `.mara/`, `.zban/`, `crates/`, `.agents/` and `.claude/`.
- Nx and its plugins are pinned at 24.0.6. The root package is `@convesoft/mara-source`, with no `version` field.
- `pnpm-workspace.yaml` allows the build script of `nx` and not that of `@swc/core`. It also exempts `@convesoft/nx-rust@1.0.0-rc.1` from pnpm's minimum release age.
- The skills live in `.agents/skills`, with `.claude/skills` linking to it.

Rejected along the way: loading nx-rust from its TypeScript source (no `test`, no cached `build`), `RUSTFLAGS` in `.env.lint`, and `build.warnings`.
