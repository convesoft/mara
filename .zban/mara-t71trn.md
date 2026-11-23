---
title: Review the decisions made in 0.4.0-rc.1
status: todo
type: task
tags:
  - maintainer
created_at: 2026-11-22T18:44:35Z
---

Goal: the maintainer reviews what the orchestrator and the workers decided on their own in the rc.1 epic.

## Decided by the maintainer

- 2026-11-22: the rewrite is published before 0.4.0, as `0.4.0-rc.1` under the npm dist-tag `next`, and the Node library and browser slice (mara-xzfeyj) becomes `0.4.0-rc.2`. A small epic resolves CI and publishing, with the zban and nx-rust repositories as references. rc.1 is to be used to migrate other repositories from the 0.3.0 format.
- 2026-11-22: tests are to run with nextest and affected selection for rapid local development, after measurements ("Agree better to gather numbers"); the full suite runs only where it is really necessary; the tests are to be decoupled from `docs/` where that is possible; a script that T3 Code runs for a new worktree copies the baseline into it; cargo-nextest and cargo-affected may be installed on the machine ("Feel free to install them").
- 2026-11-22: the tests that read this repository's docs "are not source code tests": they run beside `mara:validate`, as a project of their own with only the docs as their concern (mara-8mxw9c).
- 2026-11-22: `main` will be fast-forwarded to the integration branch, and the slice after rc.1 (mara-xzfeyj) integrates by pull requests (mara-bkqvgd).

- 2026-11-22: the npm package names are the ones 0.3.0 published (`@convesoft/mara`, `@convesoft/mara-linux-x64-gnu`, `@convesoft/mara-linux-arm64-gnu`) and the publishing jobs use the existing GitHub environment `release` ("check npm plz they were existing already").
- 2026-11-22: the CI and release workflows are specified and built in the form they keep for 0.4.0, without gates that hold only for a slice ("the CI should be almost final version unless we discover something is missing"). The rules that a real dispatch must use `next` and that `latest` is refused until mara-chjvk9 were removed from `DES-RELEASE-WORKFLOW`.

## Decided by the orchestrator

One line per decision made on the maintainer's behalf: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-22, provisional: rc.1 is for Linux x64 and arm64; macOS arm64 stays with mara-ag4ees and does not block rc.1. Rests on the macOS build of Soufflé never having been tried. Undo: block the release issue of the epic by mara-ag4ees.
- 2026-11-22, provisional: the rc.1 native package holds the CLI and `souffle` only; the Node addon and the browser package come with rc.2. Rests on neither existing yet.
- 2026-11-22: the full suite is needed on `main` after each merge, in the release workflow, where the plugin falls back to it, and on demand; `pnpm run checks` selects on a developer's machine once the switch is made. Until pull requests carry the integration, the orchestrator's check before a squash merge stays a full run. Rests on the maintainer's "run full tests only when it is really necessary" and on an affected run never standing in for a full one. Undo: `NX_RUST_TEST_MODE=full` in the `checks` script.
- 2026-11-22: cargo-nextest 0.9.152, cargo-affected 0.4.1 (with `cargo install --locked` into the Cargo home) and the `llvm-tools` component were installed on the maintainer's machine by the orchestrator, with the maintainer's permission. Undo: `cargo uninstall cargo-nextest cargo-affected`.

- 2026-11-22, provisional (mara-a57pzn): the published Linux binaries are built on `ubuntu-22.04` and `ubuntu-22.04-arm`, so the minimum glibc is 2.35, as 0.3.0 (`ADR-LINUX-BUILD-IMAGE`). Rests on keeping Debian 12 and Ubuntu 22.04. Undo: change the build images in the release workflow and the decision.
- 2026-11-22 (mara-a57pzn): `mara --version` is added to `DES-OPERATIONS` and built by mara-e8921x. Rests on the epic's "Done when" and on 0.3.0 having it. Undo: drop the line and the issue.
- 2026-11-22 (mara-a57pzn): one version and tag rule stays in the release workflow because it holds after 0.4.0 too: a version with a prerelease part is refused under `latest`. It is not in zban's workflow. Undo: remove the sentence from `DES-RELEASE-WORKFLOW`.
- 2026-11-22 (mara-a57pzn): CI runs `pnpm run checks` on `ubuntu-24.04` and `ubuntu-24.04-arm`, keeps no Nx cache between runs, caches Soufflé and Cargo's build directory, and runs the performance test on shared runners without a retry (`ADR-CI-CHECKS`). Undo: the decision and the workflow.
- 2026-11-22 (mara-a57pzn): the version is set by hand in `Cargo.toml`, not by `nx release version`; publication runs only from `main`, a dry run from any ref and on every push to `main` and the integration branch (`ADR-DISPATCHED-RELEASE`).
- 2026-11-22 (mara-a57pzn review): a P3 of the re-check is left for the packages issue: `DES-PACKAGES` says `latest` stays 0.3.0 "on every package", which a package first published with rc.2 cannot satisfy.

- 2026-11-22 (mara-8mxw9c): the test total is 856 passed (855 before): the comparison of `project init` with this repository became a test of its own in `docs-checks`. `docs-checks:test` reads the pages, `.mara/` and the templates at run time and keys `TMPDIR` only, because it starts neither the binary nor Soufflé (`ADR-NX-RUST-TASKS`). Two findings of its review are older than the change and stay in the backlog (mara-6z7bjs, mara-5c83v4).
- 2026-11-22: filed mara-jbkcxe (the workspace version becomes `0.4.0-rc.1`), which the epic had no issue for; it blocks the release workflow issue.

- 2026-11-22 (mara-e8921x): until the parser has read a complete operation name, `--version` in option position is the top-level flag (`mara project --version` is rejected); the performance harness gets no case for it; the code carries no `@mara implements` marker, because the flag is outside the document operation set (`DES-OPERATIONS`, `DES-PERFORMANCE-HARNESS`). Undo: the item text and `crates/mara/src/cli.rs`.

- 2026-11-22 (mara-397g3g): the migration steps tell the user to delete the files the conversion leaves for 0.3.0, in a commit of its own after the conversion commit, so that every state can be restored from Git (docs/migration.mara.md, "Steps"). Rests on 0.3.0 otherwise reading the converted project as one without items (read in the 0.3.0 source, not run). Undo: make step 8 optional.
- 2026-11-22 (mara-397g3g): the README installs the prerelease with `--save-exact`, and agent setups pin the MCP server and every command of this version to the exact prerelease version through one launcher; a 0.3.0 skill belongs to the 0.3.0 projects, not to the user (docs/agents.mara.md). Undo: the page text.

- 2026-11-22 (mara-7wysb0): `affectedTests` stays off and every uncached `test` task runs its full suite; the runner switches to nextest in mara-rw6rhp, which stays in this epic and blocks the release workflow issue (`ADR-NX-RUST-TASKS`). Rests on the measurements in the issue: 84 s against 159 s for the full suite with equal results; an affected run saves nothing for a Rust change (83.6 s against 84.0 s) because `mara` has a binary; the coverage baseline does not collect on this repository (494 of 825 tests fail under instrumentation; 10 GiB peak with the causes worked around). mara-azya4r (the baseline in CI) is cancelled, as the maintainer said for this outcome. Undo: mara-vxeweg (backlog) makes collection pass, then `affectedTests = true`.
- 2026-11-22, provisional (mara-svq540, question mara-6qcfn5): the licence notices of the crates in the native packages come from a generator in the repository over `cargo metadata`, not from cargo-about.

- 2026-11-22 (mara-svq540): the dispatcher runs a native package only from a `node_modules` directory on its own ancestor path and only at exactly its own version; anything else counts as not installed (`DES-PACKAGES`). It ends by the child's signal where Node can re-raise it and exits 128 + signal otherwise (mara-6y6hnn, backlog, for core-dumping signals).
- 2026-11-22 (mara-svq540): the packer reads the version and the artifact directory from `cargo metadata`, so the Pack dispatcher job of the release installs the pinned Rust toolchain though it compiles nothing; `pack-native` runs as `mara-npm:pack-native:release`, refuses `CARGO_BUILD_TARGET` and cannot see a `build.target` of a Cargo configuration file (`DES-RELEASE-WORKFLOW`). The three targets are not part of `pnpm run checks`.
- 2026-11-22 (mara-svq540): not shown before the release workflow runs: the arm64 package, the `GLIBC_` floor on the `ubuntu-22.04` image (the check fails on this machine by design), the registry mode of the install check. The dry run of mara-a3aq0n is their first evidence.

- 2026-11-22 (mara-1avrq4): CI caches the three directories cargo writes under `target/` and Cargo's registry, by name, so that the Soufflé directory cannot be in that cache; Soufflé has its own entry keyed by its build script, without a fallback (`DES-CI-WORKFLOW`, `ADR-CI-CHECKS`). `zban` is an exact devDependency, and `zban-check` keys the version and the file of the `zban` it runs. A warm run takes about 10 minutes on each image, a cold one about 17.
- 2026-11-22: reviewers' findings dropped as exotic: a local Git ignore rule that hides a zban warning from the key of `zban-check` (mara-1avrq4); a missing deadline on the MCP read of the version test (mara-e8921x).

- 2026-11-23 (mara-ezzbnc): the check names for branch protection are `Checks (ubuntu-24.04)` and `Checks (ubuntu-24.04-arm)`; no job of the release workflow is recommended as a required check, because its dry run on pull requests starts only when release inputs change and a required check that never runs blocks the merge. The maintainer sets branch protection.

- 2026-11-23 (mara-rw6rhp): the tests run with cargo-nextest 0.9.152, which CI installs from the prebuilt release archive with a pinned SHA-256 per architecture (`ADR-NX-RUST-TASKS`, `ADR-CI-CHECKS`). The full `checks` now counts 861 passed and 16 skipped (cargo: 860 and 16; the extra one is the `harness = false` binary `diagnostic_memory`). Undo: remove `testRunner` from the root `Cargo.toml` and the install step.
- 2026-11-23 (mara-rw6rhp review): two gaps in the key of `test` under nextest are accepted and stated in `ADR-NX-RUST-TASKS`, because this repository has no `.config/nextest.toml` and ignores none: an ignored `.config/nextest.toml` and the contents of nextest's user configuration are not hashed (the plugin's input declaration; backlog mara-dt2aez; reported to the maintainer as a finding about nx-rust).

- 2026-11-23 (mara-efyg6p): the failures of the session input tests on `ubuntu-22.04` were a defect of the tests, not of the product: the kernel of those images gives 1 ms timestamps, the tests checked with a clock an hour ahead, and a change within one tick left the stat unchanged. With the real clock, as every request passes it, the 2 s rule of `DES-INPUTS-AND-KEYS` saw every change (1,000 per image). The tests now set the changed path's modification time explicitly, and a new test covers the rule for a directory.
- 2026-11-23 (mara-efyg6p review): a limit of the 2 s rule is accepted and written into `DES-INPUTS-AND-KEYS`: it assumes that timestamps advance on the checker's wall clock. On a shared file system whose server clock is behind, a same-size overwrite within one server tick after a warm read can leave a stale answer (found by reading, not reproduced; backlog mara-8sdrab). Rests on that needing a network share with clock skew and a rewrite within a millisecond.
- 2026-11-23 (mara-4xsamc): the memory-limit test reaches the limit by copying memory; memory exhausted gradually by a growing relation has no test of its own any more (backlog mara-x023aa).
- 2026-11-23: the workers of mara-rw6rhp, mara-efyg6p and mara-4xsamc pushed their own branches to `origin` to see runs on GitHub's images, with a temporary trigger or probe workflow that the final commit removes. The kickoff prompt names only the workers of mara-1avrq4 and mara-a3aq0n; the orchestrator extended it and reported it. The branches `ci/github-actions-local-checks`, `test/run-tests-with-nextest`, `test/memory-limit-before-time-limit` and `probe/mara-efyg6p` are left on `origin` for the maintainer to delete.

- 2026-11-23 (mara-a3aq0n): the release workflow is `.github/workflows/release.yml` with the jobs of `DES-RELEASE-WORKFLOW`; its setup steps are a composite action shared with CI. Decided by its worker and written into the item and `ADR-DISPATCHED-RELEASE`: push and pull-request dry runs skip `npm publish --dry-run` for a version npm already has and warn (npm 11.5.1 and later refuse a dry run of a published version), a dispatch refuses such a version; runs of one ref queue, so a push cannot cancel a dispatched publication; the registry is read with `curl` on the package document, and an answer that is neither a listed version with a checksum nor a clean absence is an error; the dist-tags before the first publication are a run artifact that re-runs reuse; Publish and Tag each need the maintainer's approval in the environment `release`.
- 2026-11-23 (mara-a3aq0n): first shown by its dry runs: the arm64 native package, the `GLIBC_` 2.35 floor on the `ubuntu-22.04` images, npm 11.19.1 on the runners. Not run before the maintainer's publication: Publish, Registry check and Tag; their steps were tried by the worker with stubs and read by two reviewers.
- 2026-11-23 (mara-a3aq0n re-check, not fixed): `DES-RELEASE-WORKFLOW` and a comment in the workflow say that only push and pull-request dry runs can meet an already published version in the Dry run job; a manual dry dispatch warns and skips in that case too. The behaviour is right; the sentence is incomplete.
- 2026-11-23: a CI run on the integration branch failed once on a DNS error in the grammar download (filed as backlog; the step has no retry and no cache, by `ADR-CI-CHECKS`).

- 2026-11-23: the architecture audit after rc.1 (Gemini, at `ed7b0b9`) returned 11 findings. Filed: mara-23j2h9 under the next slice (the release workflow's publication, registry-check and tag steps as scripts with tests; a wait in Publish that hides a registry error for 30 minutes), and eight backlog issues tagged `audit` (mara-dwytrs, mara-qwj7c1, mara-b86nfg, mara-sn3qvr, mara-s25f1s, mara-09wgx8, mara-9dw13c, mara-2hz18f); notes on mara-pqhtf6, mara-kg912v and mara-j6vpbt. The wait in Publish was left as it is for rc.1: it delays a failure, it cannot publish anything wrong.

## Decisions made by the workers of the rc.1 epic

Copied from the `## Decisions made` lists of the closed issues, with the lists of their fix rounds.

### mara-a57pzn: Specify the rc.1 slice: prerelease versions on npm, CI, Linux packages and the release workflow

- The published Linux binaries are built on `ubuntu-22.04` and `ubuntu-22.04-arm`, so the minimum glibc is 2.35, as in 0.3.0 (`ADR-LINUX-BUILD-IMAGE`). The dispatcher refuses an older or missing glibc with a message.
- CI runs on `ubuntu-24.04` and `ubuntu-24.04-arm`, uses `pnpm run checks` itself, keeps no Nx cache between runs, caches Soufflé (its own entry only) and Cargo's build directory without `target/souffle-*`, fetches the grammar with `code add rust` on each run, and runs the performance test on shared runners (`ADR-CI-CHECKS`).
- `zban` becomes an exact devDependency, so that CI and local runs use the same `zban`.
- The version is set by hand in `Cargo.toml` (`ADR-DISPATCHED-RELEASE`).
- The release has the jobs Prepare, Checks, Pack dispatcher, Build, Dry run, Publish, Registry check and Tag; no package is packed before the checks pass on both platforms. Only Publish (`id-token: write`) and Tag (`contents: write`, through the job's `GITHUB_TOKEN`) can write; both run in the environment `release`, which needs the maintainer's approval, and a dry run never starts them.
- The release restores no cache. Publication runs only from `main`; every push to `main` and to the integration branch runs a dry run without a path filter. Re-running the failed jobs of the same run continues a partial publication, the GitHub release and its assets included.
- The maintainer's (2026-11-22, correcting fix round 1): both workflows have the form they keep for 0.4.0 and after, with no gate that holds only for a slice. `dist_tag` is the maintainer's input; push and pull-request dry runs use `next`. The only version–tag rule is permanent: a version with a prerelease part is never published under `latest`. The Registry check verifies every package's dist-tags after a publication. Recorded in `ADR-DISPATCHED-RELEASE`.
- The release adds three Nx targets, `pack-dispatcher`, `pack-native` and `verify-install`, none cached, and uses a `release` configuration of `mara:build`.
- The workflow file keeps 0.3.0's path, `.github/workflows/release.yml`, which its npm provenance names. GitHub generates the release notes. The package names (the 0.3.0 names) and the environment `release` were settled by the maintainer.
- The install verification also covers a daemon round trip and the dispatcher's failures, because the epic's goal names the daemon.

### mara-8mxw9c: The checks that guard this repository's docs are a project of their own, and mara:test no longer reads docs/

- `docs-checks` reads the pages, `.mara/` and the templates at run time and keys them on its `test` target. It does not use `include_str!` with `fileInputs`, which would also run its `build`, `check` and `lint` again after every page edit (recorded in ADR-NX-RUST-TASKS).
- The `test` target of `docs-checks` keys `TMPDIR`, under which its project checks write. It does not key `CI`, `MARA_NO_DAEMON`, the `MARA_TEST_*` hooks or Soufflé, because nothing in it reads them.
- The test count rises by one (856): the vocabulary check of `init.rs` is split into a product half, which keeps VER-PROJECT-INIT's real-CLI case in `mara`, and the comparison with the page, which moved.
- The crate is named `docs-checks` (Nx project `docs-checks`), at `crates/docs-checks`.

### mara-e8921x: CLI: mara --version prints the version

- DES-PERFORMANCE-HARNESS: `mara --version` adds no case without an operation. It takes no other argument, so it never prints a stats line, and `mara --version --debug-stats` is rejected on the path of the existing `mara --debug-stats --no-such-option` case. Written into the item.
- Until the parser has read a complete operation name, `--version` in option position is the top-level flag, outside the document operation set; written into DES-OPERATIONS. With other arguments, or with an inline value, it is rejected with "`mara --version` takes no value and no other argument". After an operation's words it is that operation's option (or an unknown one).
- No `@mara implements` marker: `--version` is no document operation, so it carries out no distinct obligation of REQ-CLI-MCP-PARITY, the requirement DES-OPERATIONS satisfies; the rejection runs in `parse`, which already carries that marker.

### mara-397g3g: Guidance for rc.1: README installation with the next dist-tag and the migration steps from 0.3.0

- The README installs project-locally with `--save-exact`, since a caret range on `0.4.0-rc.1` would accept later prereleases and 0.4.0 (README only; no item, it is npm behaviour).
- MCP servers for converted projects are pinned to the exact prerelease version, not `next`, as 0.3.0 pinned its version (docs/agents.mara.md).
- The migration tells users to delete `.mara/project.toml`, `.mara/schema.yaml`, the YAML rule files and `.mara/code/` after the conversion commit, in a commit of their own (step 8): the converter keeps them (DES-CONVERSION-MAPPING), no 0.4.0 operation reads them once `.mara/config.toml` exists (DES-PROJECT-CONFIGURATION), and while `project.toml` stays, a 0.3.0 tool reads the converted project as one without items instead of failing with "no project found" (verified in the v0.3.0 source, `src/lib.rs` discovery).
- The 0.3.0 skill is to be installed per 0.3.0 project, not per user (docs/agents.mara.md).
- The agent setup change (step 6) goes into the conversion commit (step 7), so one revert undoes both (docs/migration.mara.md).
- Legacy files under `.mara/` that Git does not hold are committed in step 1, ignored ones with `git add -f`, or copied out of the repository (docs/migration.mara.md).

### mara-7wysb0: Tests run with nextest and affected selection, and a full run only where it is needed: measure, then switch

- `affectedTests` stays off and each uncached `test` task runs its full suite everywhere, in `ADR-NX-RUST-TASKS`, with the measurements as the rejected alternative. The issue ends here, as its second item says; mara-azya4r, which depended on "switch", is cancelled.
- The runner switch is a follow-up (mara-rw6rhp), not part of this branch: the issue ends at a recommendation other than "switch".
- Peak memory came from a cgroup's `memory.peak`, because GNU `time` is not installed.

### mara-svq540: npm packages for rc.1: the dispatcher and the Linux native packages with the CLI and souffle

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

- The dispatcher's installation boundary and the signal behaviour are written into DES-PACKAGES; the version comes from `cargo metadata --no-deps`, with "no build metadata" as the only rule (DES-PACKAGES).
- The Pack dispatcher job installs the pinned Rust toolchain; the project is defined by `project.json`, outside the pnpm workspace; packing is offline, with atomic replacement (DES-RELEASE-WORKFLOW).
- `pack-native` follows Cargo's artifact directory and refuses `CARGO_BUILD_TARGET`. A `build.target` set in a Cargo configuration file is not detected (DES-RELEASE-WORKFLOW).
- Per-case process tracking as above (VER-INSTALL-WITHOUT-TOOLCHAIN); `mara-npm:lint` row in ADR-NX-RUST-TASKS.

### mara-1avrq4: CI: GitHub Actions runs the local checks on Linux x64 and arm64

- No step installs build tools: both images have curl, tar, sha256sum, cmake, c++, flex, bison, readelf and ninja (checked in the first run). Recorded in `DES-CI-WORKFLOW`.
- Soufflé's entry is saved by `actions/cache/restore` + `actions/cache/save` after the checks, also when a check failed. The cache action's own post-job save would skip a failed job, so the next run would build Soufflé again. Recorded in `ADR-CI-CHECKS` and `DES-CI-WORKFLOW`.
- Each push run gets its own concurrency group (`github.run_id`); a pull request's runs share `github.ref` and cancel older runs. With one group per ref, a newer push run would cancel an older one that is still pending. Recorded in `ADR-CI-CHECKS`.
- Actions are pinned by major version, as zban's and nx-rust's workflows are; neither pins by SHA. Recorded in `ADR-CI-CHECKS`.
- Job timeout 60 minutes, over three times a cold run. Recorded in `ADR-CI-CHECKS`.
- The `zban-check` key runs `zban --version` and hashes the file `zban` starts from, looked up with `node_modules/.bin` first. Recorded in the root `Cargo.toml` comment, `DES-CI-WORKFLOW` and `ADR-NX-RUST-TASKS`.
- The pnpm store is not cached: `DES-CI-WORKFLOW` names only the Soufflé and Cargo caches, and the install takes seconds.

### mara-jbkcxe: Set the workspace version to 0.4.0-rc.1

- None.

### mara-ezzbnc: The rewrite integrates by pull requests after the fast-forward of main: AGENTS.md, the template and the required checks

- The flow after the fast-forward is a section of its own, and the flow before it is a separate section that only defers to it, so that removing the "before" part later is a deletion and nothing else.
- Each paragraph of the pull request description goes on one line, because GitHub wraps the squash commit body (zban's and nx-rust's rule).
- The template has no checklist and no HTML comments, because the description becomes the commit body verbatim. The zban issue goes in a `Refs:` footer, as zban and nx-rust do it.
- The CI bullet in "Local checks" links DES-CI-WORKFLOW for the events instead of listing branches, so it stays true after the fast-forward.
- The fast-forward is the maintainer's push (mara-bkqvgd).

### mara-rw6rhp: Tests run with nextest

- `testRunner` is set in `[workspace.metadata.nx]` of the root `Cargo.toml`, not in the plugin's options in `nx.json` (`ADR-NX-RUST-TASKS`).
- CI installs nextest from its prebuilt release with a pinned SHA-256 per architecture, as nx-rust's CI does, not with `cargo install` or a third-party action (`ADR-CI-CHECKS`, `DES-CI-WORKFLOW`).
- `diagnostic_memory` follows the issue's rule for its arguments: `--list` lists, `--list --ignored` lists nothing, everything else runs.
- No `.config/nextest.toml`: the defaults ran the suite. The key holds a tracked or unignored one, but neither an ignored one nor nextest's user configuration: mara-dt2aez (backlog), for nx-rust's input declaration.

### mara-efyg6p: Session input tests with a clock an hour ahead fail on ubuntu-22.04 x64

- Tests that check with a clock an hour ahead make each change visible to a stat by setting the changed path's mtime explicitly (`visibly`, documented on the helper). No Mara item covers how unit tests fake time, so nothing else records it. Rejected alternatives:
  - sleeping past a tick, which guesses the tick;
  - another size, which does not exist for a directory;
  - the real clock in these tests, which would make their read counts depend on timing.
- No debug re-run of the release dry run's failed job: the probe on the same image answered the question.

### mara-4xsamc: The memory-limit test reaches the time limit first on slow runners

- The memory-limit case of VER-PROGRAM-FAULTS uses a program that reaches the limit by copying memory (a doubled symbol and a `cat` of 128 copies) instead of a `count` over all pairs of a `range`, so that it reaches the memory limit well before the time limit on the tested runners; recorded in the case row of VER-PROGRAM-FAULTS.

### mara-a3aq0n: Release workflow: build, pack, smoke-test and publish a prerelease from one commit

- DES-RELEASE-WORKFLOW changed: a push or pull-request dry run leaves out `npm publish --dry-run` for a version that npm has, with a warning, because npm 11.5.1 and later refuse even a dry run of a published version (`lib/commands/publish.js`; seen with npm 12.2.1); otherwise every push between a release and the next version commit would fail. A dispatch still refuses that version.
- Runs of a ref queue with `concurrency.queue: max`, so that a push to `main` cannot cancel a dispatched publication waiting behind it (GitHub's default cancels a waiting run); pull-request runs keep `single` and cancel the older one.
- The shared setup is a composite action, not a reusable workflow (a job of its own) or copied steps (drift).
- The registry is read with `curl` from its abbreviated package document, not with `npm view`, which exits 1 both for a missing version and for a failed request.
- The tarballs travel as artifacts, each Build job's install verification recorded by SHA-256; Soufflé travels as a tar archive, which keeps its mode; Publish records the dist-tags before its first publication as an artifact of the run (review round below).
- The pull-request paths also name `.github/actions/**` and `.github/scripts/**`, which the workflow runs.

- [ ] The maintainer has read the list
