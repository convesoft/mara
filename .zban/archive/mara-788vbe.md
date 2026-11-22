---
title: Move the nx-rust pin to 1.0.0-rc.2 and drop the workarounds it makes unnecessary
status: done
type: task
related:
  - mara-c4y5ts
created_at: 2026-11-22T17:37:02Z
closed_at: 2026-11-22T18:01:40Z
---

Goal: the repository runs its Nx tasks with `@convesoft/nx-rust@1.0.0-rc.2`, and every workaround that rc.2 makes unnecessary is gone.

Context: the repository pins `@convesoft/nx-rust@1.0.0-rc.1` (ADR-NX-RUST-TASKS in `docs/decisions.mara.md`). Decided by the maintainer on 2026-11-22: "I release rc2 by the way fixing issues you reported and bundled docs into plugin so you can see known limitations, let's switch to rc2". rc.2 is on npm under the dist-tag `next` (published 2026-11-22). It ships its documentation in the package: `node_modules/@convesoft/nx-rust/PLUGIN.md` and `node_modules/@convesoft/nx-rust/docs/` (start at `docs/index.mara.md`). Read those, not the plugin's checkout, which stays read-only and may be ahead of the release.

What was reported about rc.1 and what rc.2's docs say (check each against the installed package):

- `build.warnings` did nothing on `lint`, because the plugin passes `--message-format=json`. `docs/targets.mara.md` "Warnings" now says the plugin applies the setting itself in those steps, names `CARGO_BUILD_WARNINGS=deny` for one run, and says each way is in the cache key. This repository works around it with `CARGO_ENCODED_RUSTFLAGS=-Dwarnings` in `.env.lint`, and the ADR rejects `build.warnings` for the old reason.
- The keys missed tool configuration outside the repository. rc.2 keys the file that `CLIPPY_CONF_DIR` selects and warns at the start of a task about Cargo configuration and Rustfmt configuration outside the repository (`docs/caching.mara.md`, "Cargo configuration that Nx does not hash" and "Rustfmt configuration outside the repository").
- One test target: the `test` option (`docs/testing.mara.md`, "One test target").
- An input on one target stays with that target, and `inputs` without `"..."` on an inferred target now fails graph creation (`docs/configuration.mara.md`, "Changing an inferred target"). An unknown key in the plugin's tables fails graph creation too.
- `fileInputs` still refuses paths in Cargo's artifact directory; the docs name a `runtime` input on the target for files there, which is what `mara:test` and `mara:validate` do for Soufflé.
- Sharing the cache between checkouts is documented (`docs/caching.mara.md`); `nx.json` already sets `cacheDirectory`.

- [x] `package.json` pins `@convesoft/nx-rust` at exactly `1.0.0-rc.2`, `pnpm-lock.yaml` follows, and `pnpm-workspace.yaml` exempts exactly that version from pnpm's minimum release age in place of the rc.1 entry. `pnpm install --frozen-lockfile` succeeds in a fresh clone
- [x] `lint` still fails on a warning and only `lint` does: no other target and no plain `cargo` command starts to deny warnings. Use the way rc.2's docs name when it works under the installed plugin and keeps that scope (a `CARGO_BUILD_WARNINGS=deny` in `.env.lint` is the candidate); otherwise keep `CARGO_ENCODED_RUSTFLAGS` and say in the ADR why. Show it with one run: a warning added to a crate fails `mara:lint` or `mara-engine:lint`, and after the revert the earlier result is restored from the cache
- [x] Each hand-written target and each added input in the manifests' Nx tables is checked against rc.2's docs: what rc.2 now does itself is removed, and what stays is still needed. In particular, whether `mara:validate`, a hand-written target that runs cargo, can now take the plugin's inputs for a cargo command instead of naming the Cargo configuration, the toolchain file and the `CARGO_TARGET_*` variables itself. A target that is not cached and has no key to simplify (`souffle`, `test-performance`) stays as it is unless rc.2 breaks it
- [x] Graph creation prints no error and no warning of the plugin that the repository can settle. A warning about configuration outside the repository on this machine is reported, not silenced, unless a file in the repository is the documented answer; say which
- [x] `pnpm nx show projects` and the targets of `mara`, `mara-engine` and `cargo-workspace` are the same as with rc.1, or each difference is listed in the summary with its reason
- [x] ADR-NX-RUST-TASKS names rc.2, its table still gives the command each check runs as the installed plugin prints it (`pnpm nx run <target> --verbose --skip-nx-cache --output-style=static`), and its bullets about rc.1's limits are corrected. A limit that rc.2's docs still state and that this repository meets is one sentence in the ADR with the section of the plugin's docs that states it. AGENTS.md changes only if what a developer runs changes
- [x] `pnpm run checks` passes on an empty private `NX_CACHE_DIRECTORY` with the test counts of the tip (855 passed, 0 failed, 16 ignored; `project validate` 0 errors), and a second run restores from the cache what the first restored with rc.1 (9 of 12 tasks and the `wasm32` build) or the difference is explained

Not part of this issue: any change in the nx-rust checkout (a defect found in rc.2 is reported in the closing summary, never patched or worked around with new machinery), sharing one Nx cache between worktrees, `mara-5c83v4` and `mara-6z7bjs` (backlog).

Added by the maintainer on 2026-11-22: "check changelog also there is dedicated argument on test so performance once could be ran". This replaces the sentence above that `test-performance` stays as it is.

- [x] Each entry of the `v1.0.0-rc.2` release notes (#83, #87, #91, #89, #90, #92) is checked against this repository, and the closing summary says what each means here: adopted, nothing to do, or why not
- [x] The performance test runs as the configuration `performance` of `mara:test` (`test = "performance"`, `profile = "release"`), and the hand-written `test-performance` target is gone. `pnpm run checks` runs it as a command of its own with `--skip-nx-cache`, so that it is never restored and no other task runs beside it, after Soufflé is built; `checks:fast` leaves it out. A run shows that it is not restored the second time, that it has the inputs and `dependsOn` of `mara:test`, and that plain `mara:test` keeps its counts (823 in the workspace run, 32 in the performance run). `package.json`, the ADR's table and bullets and AGENTS.md "Local checks" name it

## Summary of Changes

- `@convesoft/nx-rust` pinned at `1.0.0-rc.2`; `pnpm-workspace.yaml` exempts exactly that version; the lockfile changes only the plugin's three entries. `pnpm install --frozen-lockfile` passes in a fresh folder.
- `.env.lint` sets `CARGO_BUILD_WARNINGS=deny` in place of `CARGO_ENCODED_RUSTFLAGS=-Dwarnings`. A dead-code warning added to `mara-engine` failed `mara-engine:lint` and `mara:lint`; after the revert both were restored from the cache. With the warning, `mara-engine:check` and plain `cargo clippy` passed. Without `.env.lint`, `lint` ran again instead of being restored.
- The hand-written `mara:test-performance` is replaced by the configuration `performance` of `mara:test` (`test = "performance"`, `profile = "release"`). `pnpm run checks` runs it last as `nx run mara:test:performance --skip-nx-cache`; it keeps `mara:test`'s inputs and `dependsOn` (souffle included); `mara:test` is otherwise unchanged.
- ADR-NX-RUST-TASKS: names rc.2, gives the commands as rc.2 prints them (`test` step 2 now sets `NX_RUST_TEST_PACKAGES` and `NX_RUST_TEST_RECORD`; `wasm32` selects by package ID; performance as `cargo test -p '<package ID of mara>' --test performance --profile release --locked`), corrects the bullets on tool configuration outside the repository and on warnings. AGENTS.md renames the performance check.
- Kept, still needed under rc.2: `mara`'s and `mara-engine`'s `fileInputs`, every added input and the `souffle` dependency of `mara:test`, `mara:validate`'s inputs (rc.2 gives an extra target none of the inferred inputs), `souffle`, `zban-check`.
- Graph creation prints no error and no plugin warning. This machine has no Cargo or Rustfmt configuration outside the repository, so no task warns.
- Differences of the targets against rc.1: every inferred target carries `ultracache: {mode: "off"}` (#83; matters only under Nx Cloud, which is not configured); `lint` gains a runtime input over the `clippy.toml` that `CLIPPY_CONF_DIR` selects (#90); `mara:test-performance` is gone and `mara:test` has the configuration `performance`. Projects are unchanged.
- rc.2 release notes: #83 nothing to do (no Nx Cloud); #87 used to check this change; #91 adopted (performance configuration); #89 adopted (`CARGO_BUILD_WARNINGS=deny`); #90 adopted in the ADR, root `rustfmt.toml` filed as mara-c9kja2; #92 nothing to do, only `lint` denies warnings here.

Checks: `pnpm run checks` on an empty private cache: 855 passed, 0 failed, 16 ignored; `project validate` 0 errors, 1 warning. Second run: 9 of 11 tasks restored in the first command (`souffle` and `lockfile` always run), the `wasm32` build restored, `mara:test:performance` ran again (32 passed). The tip's 9 of 12 counted `test-performance` in the first command.

## Decisions made

- `lint` denies warnings through `CARGO_BUILD_WARNINGS=deny` in `.env.lint` (ADR-NX-RUST-TASKS).
- `mara:validate` keeps naming the Cargo configuration, the toolchain file and the `CARGO_TARGET_*` variables, because rc.2 gives extra targets none of the inferred inputs (ADR-NX-RUST-TASKS).
- `checks` runs `mara:test:performance` as its last command; run by hand without `--skip-nx-cache`, Nx caches it like any configuration of `test` (ADR-NX-RUST-TASKS).
- No root `rustfmt.toml` in this change; filed as mara-c9kja2 for the maintainer.
