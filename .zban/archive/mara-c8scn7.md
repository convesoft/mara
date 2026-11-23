---
title: "0.4.0-rc.1: CI and the first npm prerelease"
status: done
type: epic
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-23T13:07:24Z
---

Goal: `npx @convesoft/mara@0.4.0-rc.1` runs the rewrite's CLI, MCP server, daemon and rule evaluation on Linux x64 and arm64 without a toolchain, built and published by a release workflow from one commit, and GitHub Actions runs the local checks.

Context: decided by the maintainer on 2026-11-22: publish the rewrite before 0.4.0, as `0.4.0-rc.1` under the npm dist-tag `next`; the Node library and browser slice (mara-xzfeyj, titled alpha.6) becomes `0.4.0-rc.2`. The maintainer wants rc.1 to migrate other repositories from the 0.3.0 `:::mara` format. `DES-PACKAGES` (docs/distribution.mara.md) still says the first npm release is 0.4.0 and that alpha slices are Git tags only, and AGENTS.md says no CI exists until the last slice: the first issue of this epic changes both. References the maintainer named: the zban repository (`/home/araketski/Work/github.com/convesoft/zban`: `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `scripts/package-npm.mjs`, `npm/zban.cjs`, `nx.json`) and the nx-rust repository (`/home/araketski/Work/github.com/convesoft/nx-rust`: `.github/workflows/`). Both are read-only.

What rc.1 does not hold: the Node addon and the browser package (rc.2), the macOS package (mara-ag4ees), the nightly budgets and the Emscripten build of Soufflé (mara-4dr2p2).

Done when:

- [x] On a Linux x64 and a Linux arm64 machine without Rust, a C++ compiler or Soufflé, `npx @convesoft/mara@0.4.0-rc.1 --version` prints the version and a rule evaluation of a small project succeeds, with npm lifecycle scripts disabled
- [x] The release workflow built every published file from one commit, after the checks passed on it, and the tag `v0.4.0-rc.1` names that commit
- [x] A push to the integration branch runs the checks in GitHub Actions and they pass
- [x] `latest` on npm is still 0.3.0

## Summary of Changes

Closed on 2026-11-23, after the maintainer's publication of `0.4.0-rc.1`.

- Published by the release workflow, run https://github.com/convesoft/mara/actions/runs/45176705041, from `main` at `a118ba8`: `@convesoft/mara`, `@convesoft/mara-linux-x64-gnu` and `@convesoft/mara-linux-arm64-gnu` at `0.4.0-rc.1` under `next`, with provenance. Its Registry check ran the install verification against the registry on `ubuntu-22.04` and `ubuntu-22.04-arm` with a toolchain-free `PATH` and `--ignore-scripts`: `mara --version`, a validation whose invariant the bundled `souffle` evaluates, MCP `initialize`, a daemon round trip, the dispatcher's failure cases and the `GLIBC_` 2.35 floor. `npx --ignore-scripts @convesoft/mara@0.4.0-rc.1 --version` prints `mara 0.4.0-rc.1` on the maintainer's machine too.
- One commit: every job checked out `a118ba8`; CI had passed on it on the release's build images (run 45173123421) and a dry dispatch had passed on it (45173194847). The annotated tag `v0.4.0-rc.1` names `a118ba8`.
- CI runs on every push to `main` and on every pull request, and passes.
- `latest` is 0.3.0 on the three packages.
- Decisions: the review issue mara-t71trn. Handoff for the next slice (mara-xzfeyj, `0.4.0-rc.2`, by pull requests against `main`): the orchestrator's state directory.
