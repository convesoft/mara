---
title: "npm packages for rc.1: the dispatcher and the Linux native packages with the CLI and souffle"
status: in-progress
type: feature
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
  - mara-e8921x
related:
  - mara-a4m588
created_at: 2026-11-22T18:44:35Z
---

Goal: a script packs `@convesoft/mara` and one native package for Linux x64 and one for Linux arm64 from built binaries, and the packed packages install and run without lifecycle scripts.

Context: `REQ-RUN-WITHOUT-TOOLCHAIN`, `DES-PACKAGES`, `ADR-SCRIPT-FREE-NPM-PACKAGES`, `DES-SOUFFLE-BUILD` and the install Verification item that the spec issue of this epic adds. Reference: zban's `scripts/package-npm.mjs` and `npm/zban.cjs` (read-only). The Linux part of the first item of mara-a4m588 and its smoke test moved here; the Node addon and the browser package stay there.

- [ ] The dispatcher package `@convesoft/mara`: a script-free launcher that finds the native package for the platform and runs its `mara` with the caller's arguments, standard streams, signals and exit status; on a platform without a native package, or when the native package is missing, it exits non-zero with a message that names the platform and what to do
- [ ] One native package per Linux platform with the `mara` binary, the `souffle` binary where `mara` looks for it first (beside its executable, `DES-SOUFFLE-BUILD`), and the licences: the repository's, Soufflé's `LICENSE`, and the notices of the crates compiled into `mara`
- [ ] Every package carries the workspace version, and the dispatcher depends on exactly that version of each native package
- [ ] The packaging script takes the built files as arguments and writes the packages to a directory it is given; it runs locally and in CI and needs no network
- [ ] The install Verification item is implemented as a script or test that runs on the packed files, in an empty directory under `$TMPDIR`, with `--ignore-scripts`: `--version`, validation of a small project, a rule evaluation with the bundled `souffle`, an MCP `initialize`. It passes locally on Linux x64
- [ ] The packed file lists are checked by name, so that a missing binary or licence fails packaging and not the user

Not part of this issue: publishing, the release workflow, the Node addon, the browser package, macOS.
