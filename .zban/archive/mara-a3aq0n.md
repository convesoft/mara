---
title: "Release workflow: build, pack, smoke-test and publish a prerelease from one commit"
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-1avrq4
  - mara-svq540
  - mara-397g3g
  - mara-jbkcxe
  - mara-rw6rhp
  - mara-efyg6p
  - mara-4xsamc
related:
  - mara-chjvk9
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-23T09:23:05Z
---

Goal: one manually started workflow builds the rc.1 packages for both Linux platforms from one commit, verifies them, and publishes them to npm under a dist-tag when the maintainer starts it without the dry run.

Context: the Design item for the release workflow that the spec issue of this epic adds. Reference: zban's `.github/workflows/release.yml` (read-only): `workflow_dispatch` with `dist_tag` and `dry_run` (default true), a check of the branch, the version read from `Cargo.toml`, refusal when the tag or the npm version exists, a build matrix, the packed packages smoke-tested locally before anything is published, npm Trusted Publishing with provenance, native packages first and the dispatcher after they are visible, a smoke test against the registry, and the tag and GitHub release last. The last item of mara-4dr2p2 (changelog and publishing) moved here for prereleases; the final release stays with mara-chjvk9.

- [x] The workflow, with a dry run as the default, runs the checks of the commit, builds `mara` in release mode and the pinned Soufflé natively on Linux x64 and arm64, packs with the packaging script, and runs the install verification on each platform
- [x] A pull request or push that changes a release input (the workflow, the packaging script, the dispatcher, the manifests) runs everything but the publication
- [x] Without the dry run it publishes the native packages, waits until npm shows them, publishes the dispatcher, runs `npx @convesoft/mara@<version> --version` against the registry, and then creates the annotated tag `v<version>` and the GitHub release. A rerun after a partial publication continues: an already published package with the same checksum is accepted, another checksum fails
- [x] It refuses the dist-tag `latest` for a prerelease version, and a version whose tag or npm version exists
- [x] The workspace version is `0.4.0-rc.1` in this change, set the way the Design item says
- [x] The issue-branch dry run passed on both platforms. Require a successful dry run on the pushed integration tip before fast-forwarding `main`.
- [x] The closing summary lists what only the maintainer can do, in order: create the native package names on npm and set their Trusted Publishers, create the GitHub environment, fast-forward `main` if the workflow must run from it, start the workflow without the dry run

Not part of this issue: starting a publication, pushing to `main`, npm or GitHub settings, the final `v0.4.0` release (mara-chjvk9).

## Summary of Changes

- `.github/workflows/release.yml`: the jobs of DES-RELEASE-WORKFLOW. Prepare records the commit, reads the version with `cargo metadata`, refuses a version that is not SemVer, a dist-tag outside `^[a-z][a-z0-9._-]*$` and `latest` for a prerelease; on a dispatch it refuses a publication from another ref than `main`, an existing tag `v<version>` and a version that npm has for any package (a push or pull-request run only warns). Checks (`ubuntu-22.04`, `ubuntu-22.04-arm`) run `pnpm run checks` and keep Soufflé as a tar archive. Pack dispatcher; Build (both images) packs the native package with `mara-npm:pack-native:release`, runs `mara-npm:verify-install` and records the SHA-256 of both tarballs it verified; Dry run; Publish, Registry check and Tag only on a dispatch without the dry run.
- Permissions: `contents: read` everywhere; `id-token: write` only in Publish, `contents: write` only in Tag, both in the environment `release`. Every checkout but Tag's has `persist-credentials: false`. Every job checks out the commit that Prepare recorded; no cache is restored and `.nx/` starts empty. Event data reaches a shell only through `env:`.
- `.github/actions/setup`: the toolchain, cargo-nextest and dependency steps that `ci.yml` and the release share. `.github/actions/release-tarballs`: Dry run, Publish and Tag accept exactly one tarball per package, each with the SHA-256 that a Build job recorded after its install verification, and write `SHA256SUMS`. `.github/scripts/npm-registry.sh`: reads npm's registry; an answer other than the package's document or "no such package" is retried and then fails, never read as absent.
- Resume: Publish skips a version that npm has with the tarball's SHA-1 and fails on another; it publishes the dispatcher once npm shows every native package (30 minutes at most). Tag accepts an existing tag only on the recorded commit, reuses an existing release, attaches what it lacks and compares attached files by SHA-256. Registry check runs `npx` with an empty cache per try for 30 minutes at most, the install verification with `--registry=<version>`, and checks that `<dist_tag>` names the version and every other dist-tag is unchanged.
- Docs: DES-RELEASE-WORKFLOW, DES-CI-WORKFLOW and ADR-DISPATCHED-RELEASE.
- The workspace version was already `0.4.0-rc.1` (`69ed042`).

Dry run: https://github.com/convesoft/mara/actions/runs/45142607574 on `407d7c2`, attempt 1, all dry-run jobs passed: Prepare; Checks (ubuntu-22.04) and Checks (ubuntu-22.04-arm), each 601 + 229 tests passed and 32 performance tests passed, `project validate` 0 errors; Pack dispatcher; Build (ubuntu-22.04) and Build (ubuntu-22.04-arm), every case of the install verification passed; Dry run. Publish, Registry check and Tag were skipped. After the review round below, https://github.com/convesoft/mara/actions/runs/45148973979 on `46d0912` passed the same way; the final commit differs from `46d0912` only by the branch removed from the push trigger and this issue file.

First shown by that run:
- The arm64 native package builds, packs and passes the install verification on `ubuntu-22.04-arm`.
- `bin/mara` and `bin/souffle` need no `GLIBC_` version above 2.35 on both `ubuntu-22.04` images.
- npm 11.19.1 with Node 24.22.3 from `actions/setup-node`, above the 11.5.1 that Trusted Publishing needs; `npm publish --dry-run` of all three tarballs under `next`.
- The tarballs' SHA-256 values agree between the two Build jobs for the dispatcher.

Not shown, first run by the maintainer: Publish, Registry check and Tag. The install verification's `--registry` mode was tried locally against the published 0.3.0 through `nx run mara-npm:verify-install --registry=0.3.0`: Nx forwards the flag, npm installs from the registry, `--version` and the `GLIBC_` cases pass; the other cases fail on 0.3.0's older CLI.

Earlier run https://github.com/convesoft/mara/actions/runs/45129288351 on `d185c3c` failed Checks (ubuntu-22.04) three times on tests outside this issue: mara-efyg6p and mara-4xsamc, now fixed.

## Decisions made

- DES-RELEASE-WORKFLOW changed: a push or pull-request dry run leaves out `npm publish --dry-run` for a version that npm has, with a warning, because npm 11.5.1 and later refuse even a dry run of a published version (`lib/commands/publish.js`; seen with npm 12.2.1); otherwise every push between a release and the next version commit would fail. A dispatch still refuses that version.
- Runs of a ref queue with `concurrency.queue: max`, so that a push to `main` cannot cancel a dispatched publication waiting behind it (GitHub's default cancels a waiting run); pull-request runs keep `single` and cancel the older one.
- The shared setup is a composite action, not a reusable workflow (a job of its own) or copied steps (drift).
- The registry is read with `curl` from its abbreviated package document, not with `npm view`, which exits 1 both for a missing version and for a failed request.
- The tarballs travel as artifacts, each Build job's install verification recorded by SHA-256; Soufflé travels as a tar archive, which keeps its mode; Publish records the dist-tags before its first publication as an artifact of the run (review round below).
- The pull-request paths also name `.github/actions/**` and `.github/scripts/**`, which the workflow runs.

## What only the maintainer can do, in order

1. Confirm that the Trusted Publisher entries of `@convesoft/mara`, `@convesoft/mara-linux-x64-gnu` and `@convesoft/mara-linux-arm64-gnu` name `convesoft/mara`, the workflow `release.yml` and the environment `release`.
2. Fast-forward `main` to the integration branch once a dry run has passed on its tip. Needed: "This trigger only receives events when the workflow file is on the default branch" (GitHub's workflow syntax, `on.workflow_dispatch`), and `origin/main`'s `release.yml` has no `workflow_dispatch`, so nobody can dispatch the new workflow on any ref until then.
3. Set branch protection with the required checks `Checks (ubuntu-24.04)` and `Checks (ubuntu-24.04-arm)`.
4. Dispatch Release from `main` with `dist_tag` `next` and `dry_run` off, and approve it in the environment `release` twice: Publish, then Tag.
5. Check `npx @convesoft/mara@0.4.0-rc.1 --version` and that `latest` is still 0.3.0 on every package.

After the publication, push dry runs on `main` warn that the version is on npm until the next version commit.

## Review round 1

Fixed at `46d0912` after the review of `6407ce8`; DES-RELEASE-WORKFLOW and ADR-DISPATCHED-RELEASE follow each change.

- Dry run reads the registry for each package right before its `npm publish --dry-run`, and again when npm refuses it: a version that npm has by then is left out with a warning, any other refusal fails.
- `.github/scripts/npm-registry.sh` is a command, not sourced, and changes no option of the step. A version is present only with a 40-digit SHA-1; absent only when the package's document lacks it or the registry answers 404 with `{"error":"Not found"}`; anything else, a listed version without a SHA-1 included, is retried and fails. Each request stops after 30 s, each `npx` try after 5 minutes.
- The dist-tag baseline is the artifact `dist-tags-before`, uploaded before the first `npm publish` and kept by a re-run; a re-run that finds none records the tags then, since no attempt published, and its upload fails if the artifact exists. Registry check compares the other dist-tags as JSON values, in any order.
- Tag looks the release up through the REST API (404 is none), attaches again an asset not in the state `uploaded` (https://docs.github.com/en/rest/releases/assets#upload-a-release-asset: a 502 "may leave an empty asset with a state of starter. It can be safely deleted."), and compares uploaded assets by `digest`, else by download.
- A dispatch validates `dist_tag` as given and refuses an empty one; only push and pull-request runs use `next`.

Tried without publishing: the helper against a local server's plain-JSON answers (present, absent version, 404 "Not found", a listed version without or with an empty `dist.shasum`, no `versions`, another package's document, no JSON, 404 with another body, 503) and against npm for `@convesoft/mara@0.3.0`, an absent `0.4.0-rc.1` and an absent package; the dist-tag step on the same map in another order (passes) and a moved `latest` (fails); the Dry run step with a stub npm (published before, published during npm's dry run, refused while absent, registry error); the Tag step with a fake release and a stub `gh` (matching digest, missing file, `starter` and `open` assets, no digest, differing digest or download, 404 then created, 503); Prepare with an empty dispatch input.

Local `pnpm run checks` on a fresh `NX_CACHE_DIRECTORY` at `46d0912`: exit 0, 830 passed and 14 skipped, 32 passed and 2 skipped, `project validate` 0 errors, 0 warnings. Dry run 45148973979: Prepare, both Checks (601 + 229 and 32 tests each), Pack dispatcher, both Builds (13 of 13 install-verification cases, the `GLIBC_` cases included), Dry run (npm 11.19.1, three `npm publish --dry-run` under `next`) passed; Publish, Registry check and Tag skipped.
