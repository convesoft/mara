---
title: The release dry run passes on main with the packages of 0.4.0-rc.2
status: done
type: task
parent: mara-xzfeyj
blocked_by:
  - mara-865a2e
  - mara-gn76br
  - mara-23j2h9
created_at: 2026-11-25T10:18:52Z
closed_at: 2026-11-25T18:30:35Z
---

Goal: before the maintainer publishes `0.4.0-rc.2`, one dry dispatch of the release workflow passes on the commit of `main` that is then published (`DES-RELEASE-WORKFLOW`, docs/distribution.mara.md).

Context: no merge runs the release build, the packing or the install verification (mara-9r3bqr). The orchestrator does this issue itself, when every other issue of mara-xzfeyj is closed and CI's run on that commit of `main` has passed: `gh workflow run release.yml --ref main -f dist_tag=next -f dry_run=true`. No worker starts it.

- [x] CI's push run on the commit passed
- [x] The dry dispatch on that commit passed: Prepare, Pack dispatcher, both Builds with the install verification, Dry run; its log lists every package of rc.2 and the notes from `v0.4.0-rc.1`
- [x] The run's URL and the commit are recorded here and reported to the maintainer

## Summary of Verification

- Commit on `main`: `4cbcf265c21fa120f8cab13e304cf15506a6c3ca`
- `main` push CI run: https://github.com/convesoft/mara/actions/runs/45670123918 — both `Checks (ubuntu-22.04)` and `Checks (ubuntu-22.04-arm)` passed (arm64 passed on rerun after one `library_timing` flake filed as mara-zfn4jg under `mara-qzp1gd`).
- `main` release dry dispatch (`dist_tag=next`, `dry_run=true`): https://github.com/convesoft/mara/actions/runs/45670276066 — Prepare, Pack dispatcher, `Build (ubuntu-22.04)`, `Build (ubuntu-22.04-arm)`, and Dry run all passed; Publish, Registry check, and Tag were skipped.
- Packages listed in Dry run: `@convesoft/mara-linux-x64-gnu` (8 files, 23.6 MB / 78.8 MB), `@convesoft/mara-linux-arm64-gnu` (8 files, 22.6 MB / 68.4 MB), and `@convesoft/mara` (52 files, 2.8 MB / 11.4 MB); release notes generated cleanly from `v0.4.0-rc.1` covering #83, #85, #88, #89, #91, and #93.
