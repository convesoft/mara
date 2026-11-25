---
title: The release workflow's publication, registry-check and tag steps are scripts with tests in the repository
status: in-progress
type: task
parent: mara-xzfeyj
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: what the release workflow does after the dry run can be run and tested outside a workflow, before the next publication uses it.

Context: found by the architecture audit after rc.1 (commit ed7b0b9). `.github/workflows/release.yml` holds about 330 lines of shell, jq and inline Node (`DES-RELEASE-WORKFLOW`, docs/distribution.mara.md). No dry run executes Publish, Registry check or Tag, and neither they nor `.github/scripts/npm-registry.sh` nor `.github/actions/release-tarballs` has a test in the repository; the worker of mara-a3aq0n tried them with stubs outside it. Defects the audit found by reading:

- Publish, the wait for the native packages (`until [ "$("$registry" shasum …)" = "$expected" ]`): inside a loop condition bash does not stop on the helper's failure, so a registry error, or another checksum, reads as "npm does not show it yet" for 30 minutes and then fails with that message.
- `release-tarballs/action.yml` reads the checksum file's lines in two different ways (`cut -d ' ' -f 3-` and `awk 'substr($0, 67) == name'`).
- `DES-RELEASE-WORKFLOW` says Prepare records which packages npm has the version of; Prepare has no such output and Dry run reads npm itself.
- Build requires exactly two tarballs, which the browser package of rc.2 changes.

- [ ] The release operations (prepare and validate, collect the verified tarballs, dry run, publish and resume, registry check, tag and GitHub release) are scripts that the workflow's steps call; the workflow keeps conditions, permissions and artifacts
- [ ] Tests in the repository run them against loopback stubs of the npm registry and GitHub's API and a temporary bare Git remote: resume after a partial publication, a different checksum fails at once, a registry error fails and is never read as absence, the dist-tag baseline, an existing tag, an asset in the state `starter`
- [ ] The wait for the native packages fails at once on a registry error or another checksum
- [ ] The number and names of the packages come from one table, so that a new package changes no workflow step
- [ ] `DES-RELEASE-WORKFLOW` agrees with what Prepare records
