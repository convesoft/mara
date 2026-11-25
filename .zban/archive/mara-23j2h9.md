---
title: The release workflow's publication, registry-check and tag steps are scripts with tests in the repository
status: done
type: task
parent: mara-xzfeyj
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
closed_at: 2026-11-25T10:55:19Z
---

Goal: what the release workflow does after the dry run can be run and tested outside a workflow, before the next publication uses it.

Context: found by the architecture audit after rc.1 (commit ed7b0b9). `.github/workflows/release.yml` holds about 330 lines of shell, jq and inline Node (`DES-RELEASE-WORKFLOW`, docs/distribution.mara.md). No dry run executes Publish, Registry check or Tag, and neither they nor `.github/scripts/npm-registry.sh` nor `.github/actions/release-tarballs` has a test in the repository; the worker of mara-a3aq0n tried them with stubs outside it. Defects the audit found by reading:

- Publish, the wait for the native packages (`until [ "$("$registry" shasum …)" = "$expected" ]`): inside a loop condition bash does not stop on the helper's failure, so a registry error, or another checksum, reads as "npm does not show it yet" for 30 minutes and then fails with that message.
- `release-tarballs/action.yml` reads the checksum file's lines in two different ways (`cut -d ' ' -f 3-` and `awk 'substr($0, 67) == name'`).
- `DES-RELEASE-WORKFLOW` says Prepare records which packages npm has the version of; Prepare has no such output and Dry run reads npm itself.
- Build requires exactly two tarballs, which the browser package of rc.2 changes.

- [x] The release operations (prepare and validate, collect the verified tarballs, dry run, publish and resume, registry check, tag and GitHub release) are scripts that the workflow's steps call; the workflow keeps conditions, permissions and artifacts
- [x] Tests in the repository run them against loopback stubs of the npm registry and GitHub's API and a temporary bare Git remote: resume after a partial publication, a different checksum fails at once, a registry error fails and is never read as absence, the dist-tag baseline, an existing tag, an asset in the state `starter`
- [x] The wait for the native packages fails at once on a registry error or another checksum
- [x] The number and names of the packages come from one table, so that a new package changes no workflow step
- [x] `DES-RELEASE-WORKFLOW` agrees with what Prepare records

## Summary of Changes

- Every step of `release.yml` that did more than run a target or an action now calls a Bash script of `.github/scripts`: `release-prepare.sh`, `release-tarballs.sh` (`record` for Build, `collect` for `.github/actions/release-tarballs`), `release-dry-run.sh`, `release-dist-tags.sh` (`record` for Publish, `check` for Registry check), `release-publish.sh`, `release-npx-check.sh`, `release-tag.sh`, `release-attach.sh`. Jobs, their order, permissions, conditions, the environment `release`, artifacts and Trusted Publishing without a token are unchanged.
- `releasePackages(version)` in `packages/mara-npm/scripts/packages.mjs` is the release's package table: the native packages of the dispatcher's `platforms`, then `anyPlatformPackages` (the dispatcher last). Prepare outputs `{name, native, tarball}`; Build's record, the collect check, Dry run, Publish and the dist-tags steps read `native` instead of "two tarballs", `.[:-1]` and `last`. Prepare's `dispatcher_tarball` output is gone; Pack dispatcher uploads `dist/npm/*.tgz`.
- `mara-npm:test` (`node --test "packages/mara-npm/tests/*.test.mjs"`, not cached) runs 69 tests of the real scripts with the real npm, git, curl, gh, jq and cargo against loopback stubs of npm's registry and GitHub's API and a temporary bare remote. `nx release changelog` is stood in for by a `pnpm` stub.
- Docs: DES-RELEASE-WORKFLOW (Prepare records no npm state, Build's record, Publish's and Registry check's fail-fast waits, the scripts, the test variables, the `test` target), DES-PACKAGES (the table sentence), ADR-NX-RUST-TASKS (`mara-npm:test` row), ADR-DISPATCHED-RELEASE (decisions below).

Defects of the issue, as the workflow stood at cfa4c85:

- The native-package wait (`until [ "$(… shasum …)" = "$expected" ]`) still read a registry error or another SHA-1 as "not yet": now fails at once. Registry check's dist-tag wait (`until tags=$(…) && …`) had the same defect: a registry error now fails at once.
- `release-tarballs` read the records two ways: now one parser, with a format check of each line.
- DES said Prepare records which packages npm has the version of: corrected; it refuses such a version and records the packages of the table.
- Build's "exactly two tarballs": now the tarballs for any platform and exactly one native one.
- The Tag job no longer creates the tag by hand (cfa4c85, `nx release changelog`); its re-run rules are what `release-tag.sh` carries and the tests cover.

Intended changes of behaviour:

- npm's version check runs inside the publish script, after the dist-tags baseline is recorded instead of before it (the baseline is read-only and no publication precedes it).
- Prepare's branches for a push or pull-request event are gone: only a dispatch starts the workflow.
- With `NPM_REGISTRY` set (tests only), `npm publish`, its dry run and `npx` get `--registry`; `RELEASE_POLL_SECONDS`, `RELEASE_WAIT_SECONDS`, `NPM_REGISTRY_RETRY_SECONDS` change pauses and limits. The workflow sets none.

Branches no test reaches: the usage errors; curl failing to connect in Tag and attach (`set -e` ends the script); `gh` failing in attach; `timeout 300` of an npx try; a missing baseline file in `check`; and everything the workflow keeps (artifacts, conditions, OIDC, provenance generation, the real `nx release changelog`).

## Decisions made

- The release steps are Bash scripts in `.github/scripts`, moved from the workflow; rejected: inline shell (zban, nx-rust), a Node rewrite (ADR-DISPATCHED-RELEASE).
- Publish passes `--provenance` to the publish script as an argument; rejected: the flag inside the script, which no test could publish with (ADR-DISPATCHED-RELEASE).
- The package table is `releasePackages`/`anyPlatformPackages` in `packages.mjs`; rejected: a second list, and matrices from the table (ADR-DISPATCHED-RELEASE, DES-PACKAGES).
- The tests reach GitHub's API as `gh` reaches `github.localhost`, through the stub as HTTP proxy; rejected: an HTTPS stub, replacing gh with curl (ADR-DISPATCHED-RELEASE).
- `mara-npm:test` is not cached: it runs the machine's npm, git, curl, gh, jq and cargo (ADR-NX-RUST-TASKS).
- The scripts take `NPM_REGISTRY`, `NPM_REGISTRY_RETRY_SECONDS`, `RELEASE_POLL_SECONDS` and `RELEASE_WAIT_SECONDS` for the tests (DES-RELEASE-WORKFLOW).

Checks: `pnpm run checks` on the merge with origin/main (5040d28), fresh Nx cache: 932 passed, 0 failed, 16 skipped (831 and 14 in the workspace nextest run, 69 and 0 in `mara-npm:test`, 32 and 2 in the performance run); `project validate` 0 errors, 0 warnings. Probes: with the old `until` wait restored, two wait tests fail; with Nx given the caller's standard input, the Tag test fails. The dry dispatch of the release workflow is owed until `main` carries `0.4.0-rc.2`.
