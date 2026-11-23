---
title: "Release workflow: build, pack, smoke-test and publish a prerelease from one commit"
status: in-progress
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
---

Goal: one manually started workflow builds the rc.1 packages for both Linux platforms from one commit, verifies them, and publishes them to npm under a dist-tag when the maintainer starts it without the dry run.

Context: the Design item for the release workflow that the spec issue of this epic adds. Reference: zban's `.github/workflows/release.yml` (read-only): `workflow_dispatch` with `dist_tag` and `dry_run` (default true), a check of the branch, the version read from `Cargo.toml`, refusal when the tag or the npm version exists, a build matrix, the packed packages smoke-tested locally before anything is published, npm Trusted Publishing with provenance, native packages first and the dispatcher after they are visible, a smoke test against the registry, and the tag and GitHub release last. The last item of mara-4dr2p2 (changelog and publishing) moved here for prereleases; the final release stays with mara-chjvk9.

- [ ] The workflow, with a dry run as the default, runs the checks of the commit, builds `mara` in release mode and the pinned Soufflé natively on Linux x64 and arm64, packs with the packaging script, and runs the install verification on each platform
- [ ] A pull request or push that changes a release input (the workflow, the packaging script, the dispatcher, the manifests) runs everything but the publication
- [ ] Without the dry run it publishes the native packages, waits until npm shows them, publishes the dispatcher, runs `npx @convesoft/mara@<version> --version` against the registry, and then creates the annotated tag `v<version>` and the GitHub release. A rerun after a partial publication continues: an already published package with the same checksum is accepted, another checksum fails
- [ ] It refuses the dist-tag `latest` for a prerelease version, and a version whose tag or npm version exists
- [ ] The workspace version is `0.4.0-rc.1` in this change, set the way the Design item says
- [ ] A dry run on the pushed integration branch passes on both platforms; its link is in the closing summary
- [ ] The closing summary lists what only the maintainer can do, in order: create the native package names on npm and set their Trusted Publishers, create the GitHub environment, fast-forward `main` if the workflow must run from it, start the workflow without the dry run

Not part of this issue: starting a publication, pushing to `main`, npm or GitHub settings, the final `v0.4.0` release (mara-chjvk9).
