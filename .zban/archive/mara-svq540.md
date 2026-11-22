---
title: "npm packages for rc.1: the dispatcher and the Linux native packages with the CLI and souffle"
status: done
type: feature
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
  - mara-e8921x
related:
  - mara-a4m588
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T21:47:02Z
---

Goal: a script packs `@convesoft/mara` and one native package for Linux x64 and one for Linux arm64 from built binaries, and the packed packages install and run without lifecycle scripts.

Context: `REQ-RUN-WITHOUT-TOOLCHAIN`, `DES-PACKAGES`, `ADR-SCRIPT-FREE-NPM-PACKAGES`, `DES-SOUFFLE-BUILD` and the install Verification item that the spec issue of this epic adds. Reference: zban's `scripts/package-npm.mjs` and `npm/zban.cjs` (read-only). The Linux part of the first item of mara-a4m588 and its smoke test moved here; the Node addon and the browser package stay there.

- [x] The dispatcher package `@convesoft/mara`: a script-free launcher that finds the native package for the platform and runs its `mara` with the caller's arguments, standard streams, signals and exit status; on a platform without a native package, or when the native package is missing, it exits non-zero with a message that names the platform and what to do
- [x] One native package per Linux platform with the `mara` binary, the `souffle` binary where `mara` looks for it first (beside its executable, `DES-SOUFFLE-BUILD`), and the licences: the repository's, Soufflé's `LICENSE`, and the notices of the crates compiled into `mara`
- [x] Every package carries the workspace version, and the dispatcher depends on exactly that version of each native package
- [x] The packaging script takes the built files as arguments and writes the packages to a directory it is given; it runs locally and in CI and needs no network
- [x] The install Verification item is implemented as a script or test that runs on the packed files, in an empty directory under `$TMPDIR`, with `--ignore-scripts`: `--version`, validation of a small project, a rule evaluation with the bundled `souffle`, an MCP `initialize`. It passes locally on Linux x64
- [x] The packed file lists are checked by name, so that a missing binary or licence fails packaging and not the user

Not part of this issue: publishing, the release workflow, the Node addon, the browser package, macOS.

## Summary of Changes

- `packages/mara-npm`, the Nx project `mara-npm` (a `project.json`, no `package.json`, so no change to `pnpm-lock.yaml` or `pnpm-workspace.yaml`):
  - `bin/mara.cjs`, the dispatcher. One table of platforms (Linux x64 and arm64, glibc). It refuses an unsupported platform, no glibc and a glibc older than 2.35 with the messages of DES-PACKAGES. It resolves the native package by Node's module resolution from its own file, accepts it only at exactly its own version, and spawns its `bin/mara` without a shell, with the caller's arguments, environment and stdio. It forwards SIGINT, SIGTERM and SIGHUP, and passes on the exit status, or ends by the same signal (128 plus the signal number where Node cannot end by it: SIGPIPE, SIGUSR1).
  - `scripts/pack.mjs dispatcher <out>` and `native <mara> <souffle> <souffle-licence> <out>`: the version, repository and artifact directory from `cargo metadata --no-deps` (corrected in fix round 1; the Pack dispatcher job installs the pinned Rust toolchain, as DES-RELEASE-WORKFLOW says); `native` checks that `mara --version` prints the workspace version; stages, runs `npm pack --ignore-scripts`, and checks the tarball's file list by name and mode (`tar -tvzf`), deleting a wrong tarball.
  - `scripts/third-party-licenses.mjs`: `THIRD-PARTY-LICENSES` from `cargo metadata --locked --offline --filter-platform <host>`: every normal and build dependency of `mara`, transitively, without workspace crates (188 crates, 117 distinct texts, 381 kB). A crate without a licence file must be in the table `licenceFilesOf`, which names the crate of its upstream repository whose files apply, with the same declared licence (14 Wasmtime-internal crates map to `wasmtime`); any other fails packaging and names the crate.
  - `scripts/verify-install.mjs`: VER-INSTALL-WITHOUT-TOOLCHAIN on the tarballs in `dist/npm` (offline npm, empty private cache, `--ignore-scripts`) or with `--registry=<version>`. The runtime cases run with only `PATH` (links to node and npm), a new `HOME` and a fresh `XDG_RUNTIME_DIR` of mode 0700 directly under the temporary directory. A final case fails, and ends the processes, when any process naming a file of the verification still runs.
  - `scripts/packages.mjs`: the parts the two scripts share (file lists with modes, the platform table, the version, the tarball check).
  - Targets `pack-dispatcher`, `pack-native` and `verify-install`, all `cache: false`. `pack-native` packs only under its configuration `release`, which selects `mara:build:release`; without it, it fails before packing.
- `crates/mara/Cargo.toml`: the configuration `release` of `mara:build` (`profile = "release"`).
- DES-PACKAGES, DES-RELEASE-WORKFLOW, VER-INSTALL-WITHOUT-TOOLCHAIN updated (see Decisions made).

Checks:
- `pnpm run checks` (berth --exclusive): exit 0. 860 passed, 0 failed, 16 ignored; 15 tasks in the first `nx` command (the new one is `mara-npm:lint`).
- `nx run mara-npm:pack-dispatcher`, `nx run mara-npm:pack-native:release`, `nx run mara-npm:verify-install` on Linux x64. Every Test case passes. The Analysis case fails locally, as expected: the build host has glibc 2.44, so `bin/mara` needs GLIBC_2.39 and `bin/souffle` needs GLIBC_2.38. It can only pass on the release's `ubuntu-22.04` image, which this machine cannot show.
- By hand: `nx run mara-npm:pack-native` without the configuration exits 1 with its message. A missing input fails packing and names it. The tarball check reports missing, unexpected and wrong-mode files. Through the dispatcher, exit statuses 1 and 2 pass through, and SIGTERM to the dispatcher reaches `mara`, after which the dispatcher ends by SIGTERM (143).
- Not shown here: the arm64 package (not cross-compiled; the release workflow packs it on `ubuntu-22.04-arm`), the registry mode of `verify-install`, and npm 11's behaviour (local npm is 12.2.1; the scripts do not use npm's JSON output, whose shape changed in 12).

Incident: a hand-run `mara project validate` of mine, without `--no-daemon` or a private `XDG_RUNTIME_DIR`, started a daemon in `/run/user/1000/mara` (PID 1171161). I stopped it with `mara daemon stop` and removed the directory, which then held only its lock file. The install verification was not the cause; it now also checks that no process of its own outlives it.

## Decisions made

- The project is `packages/mara-npm` (Nx name `mara-npm`), defined by `project.json`, so no pnpm workspace member and no lockfile change. DES-RELEASE-WORKFLOW names it, also in the pull-request paths.
- `pack-native` runs as `nx run mara-npm:pack-native:release`. Nx 24's `dependsOn` cannot select a dependency's configuration: a dependency gets the configuration of the command (`create-task-graph.js`), not the target's default. Without the configuration the target fails before packing, so it never packs a binary that Nx did not just build. Rejected: `defaultConfiguration` (does not propagate), forwarding options (would forward `command` to nx-rust). DES-RELEASE-WORKFLOW.
- `THIRD-PARTY-LICENSES` comes from `cargo metadata`, not cargo-about, which is not installed here (installing a cargo tool is the maintainer's call). It covers normal and build dependencies, a superset of what is compiled in. A crate without a licence file needs an explicit table entry; matching by repository URL was rejected because three Wasmtime crates declare none. DES-PACKAGES.
- The dispatcher accepts the native package only at exactly its own version; another version is the "not installed" case. DES-PACKAGES.
- The dispatcher's platform table is the one list of native packages; the packaging script reads it. DES-PACKAGES.
- `latest` wording: "every package that 0.3.0 published". DES-PACKAGES.
- The install verification and the pack targets are not part of `pnpm run checks`: they need a release build and write tarballs. DES-RELEASE-WORKFLOW.
- The GLIBC check is a case of the install verification, not a separate Build step. DES-RELEASE-WORKFLOW and VER-INSTALL-WITHOUT-TOOLCHAIN.
- Tarballs install with npm's `--offline` and an empty private cache, so npm fetches nothing; the arm64 optional dependency is skipped. VER-INSTALL-WITHOUT-TOOLCHAIN.
- A final case fails when a process of the verification outlives it. VER-INSTALL-WITHOUT-TOOLCHAIN.
- The packaging and verification scripts read the version with `cargo metadata --no-deps`, so the Pack dispatcher job installs the pinned Rust toolchain, although nothing is compiled; nx-rust needs cargo for the project graph too (DES-RELEASE-WORKFLOW; corrected in fix round 1).

Rejected approaches: the repository-URL heuristic for licence files; reading `npm pack --json`, whose shape npm 12 changed from npm 11.

Follow-ups: mara-s6dn0x (JS formatting is not a local check), mara-6y6hnn (the dispatcher re-raises a core-dumping signal on Node).

## Fix round 1 (review of e58204b)

- The dispatcher takes the native package only from a `node_modules` folder on its own ancestor path (Node's `require.resolve.paths`, without the global folders and `NODE_PATH`), links resolved; a package found only elsewhere is "not installed". New verification case: a native package reachable only through `NODE_PATH` is refused. With the e58204b dispatcher the same setup ran `mara`.
- `pack.mjs native <souffle> <souffle-licence> <out>` reads `mara` from `release/mara` in the artifact directory that `cargo metadata` names, so it packs what `mara:build:release` built under `CARGO_TARGET_DIR`; it fails with `CARGO_BUILD_TARGET` set, and names the path when the build is missing.
- The version, repository and artifact directory come from `cargo metadata --no-deps`; the only check on the version is that it has no build metadata. The text parser and the SemVer grammar are gone.
- npm pack runs with `--offline --no-update-notifier` and an empty private cache, into a temporary folder in the output directory; the checked tarball is renamed over the old one, and a failure removes only what the run created.
- The tarball check reads member names with `tar -tzf` (spaces survive), modes with `tar -tvzf` in the same order, and fails on a repeated member.
- The verifier checks after every case what the case started: unreaped children, and daemons by the PID their status reports while that PID's executable lies in the verification directory. It ends a leftover with SIGTERM, waits for it to end and fails the case. Any other process whose executable lies there is reported and never signalled. Children of `spawnSync` are never signalled: they have been reaped, so their PIDs can be reused.
- `NODE_OPTIONS` quotes the preload path; the file-list and GLIBC cases print their command and output (for GLIBC, the lines that name a version above 2.35); the MCP case settles on `close` or on `error`.

### Decisions made

- The dispatcher's installation boundary and the signal behaviour are written into DES-PACKAGES; the version comes from `cargo metadata --no-deps`, with "no build metadata" as the only rule (DES-PACKAGES).
- The Pack dispatcher job installs the pinned Rust toolchain; the project is defined by `project.json`, outside the pnpm workspace; packing is offline, with atomic replacement (DES-RELEASE-WORKFLOW).
- `pack-native` follows Cargo's artifact directory and refuses `CARGO_BUILD_TARGET`. A `build.target` set in a Cargo configuration file is not detected (DES-RELEASE-WORKFLOW).
- Per-case process tracking as above (VER-INSTALL-WITHOUT-TOOLCHAIN); `mara-npm:lint` row in ADR-NX-RUST-TASKS.
