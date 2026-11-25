---
title: The release dry run passes on main with the packages of 0.4.0-rc.2
status: todo
type: task
parent: mara-xzfeyj
blocked_by:
  - mara-865a2e
  - mara-gn76br
  - mara-23j2h9
created_at: 2026-11-25T10:18:52Z
---

Goal: before the maintainer publishes `0.4.0-rc.2`, one dry dispatch of the release workflow passes on the commit of `main` that is then published (`DES-RELEASE-WORKFLOW`, docs/distribution.mara.md).

Context: no merge runs the release build, the packing or the install verification (mara-9r3bqr). The orchestrator does this issue itself, when every other issue of mara-xzfeyj is closed and CI's run on that commit of `main` has passed: `gh workflow run release.yml --ref main -f dist_tag=next -f dry_run=true`. No worker starts it.

- [ ] CI's push run on the commit passed
- [ ] The dry dispatch on that commit passed: Prepare, Pack dispatcher, both Builds with the install verification, Dry run; its log lists every package of rc.2 and the notes from `v0.4.0-rc.1`
- [ ] The run's URL and the commit are recorded here and reported to the maintainer
