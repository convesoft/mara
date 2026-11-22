---
title: Move the nx-rust pin to 1.0.0-rc.2 and drop the workarounds it makes unnecessary
status: in-progress
type: task
related:
  - mara-c4y5ts
created_at: 2026-11-22T17:37:02Z
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

- [ ] `package.json` pins `@convesoft/nx-rust` at exactly `1.0.0-rc.2`, `pnpm-lock.yaml` follows, and `pnpm-workspace.yaml` exempts exactly that version from pnpm's minimum release age in place of the rc.1 entry. `pnpm install --frozen-lockfile` succeeds in a fresh clone
- [ ] `lint` still fails on a warning and only `lint` does: no other target and no plain `cargo` command starts to deny warnings. Use the way rc.2's docs name when it works under the installed plugin and keeps that scope (a `CARGO_BUILD_WARNINGS=deny` in `.env.lint` is the candidate); otherwise keep `CARGO_ENCODED_RUSTFLAGS` and say in the ADR why. Show it with one run: a warning added to a crate fails `mara:lint` or `mara-engine:lint`, and after the revert the earlier result is restored from the cache
- [ ] Each hand-written target and each added input in the manifests' Nx tables is checked against rc.2's docs: what rc.2 now does itself is removed, and what stays is still needed. In particular, whether `mara:validate`, a hand-written target that runs cargo, can now take the plugin's inputs for a cargo command instead of naming the Cargo configuration, the toolchain file and the `CARGO_TARGET_*` variables itself. A target that is not cached and has no key to simplify (`souffle`, `test-performance`) stays as it is unless rc.2 breaks it
- [ ] Graph creation prints no error and no warning of the plugin that the repository can settle. A warning about configuration outside the repository on this machine is reported, not silenced, unless a file in the repository is the documented answer; say which
- [ ] `pnpm nx show projects` and the targets of `mara`, `mara-engine` and `cargo-workspace` are the same as with rc.1, or each difference is listed in the summary with its reason
- [ ] ADR-NX-RUST-TASKS names rc.2, its table still gives the command each check runs as the installed plugin prints it (`pnpm nx run <target> --verbose --skip-nx-cache --output-style=static`), and its bullets about rc.1's limits are corrected. A limit that rc.2's docs still state and that this repository meets is one sentence in the ADR with the section of the plugin's docs that states it. AGENTS.md changes only if what a developer runs changes
- [ ] `pnpm run checks` passes on an empty private `NX_CACHE_DIRECTORY` with the test counts of the tip (855 passed, 0 failed, 16 ignored; `project validate` 0 errors), and a second run restores from the cache what the first restored with rc.1 (9 of 12 tasks and the `wasm32` build) or the difference is explained

Not part of this issue: any change in the nx-rust checkout (a defect found in rc.2 is reported in the closing summary, never patched or worked around with new machinery), sharing one Nx cache between worktrees, `mara-5c83v4` and `mara-6z7bjs` (backlog).
