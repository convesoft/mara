---
title: The release steps as scripts with tests, where zban and nx-rust keep their release shell in the workflow
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
discovered_from: mara-23j2h9
created_at: 2026-11-25T11:40:26Z
---

Goal: the maintainer confirms or overturns a departure from the reference repositories in how the release workflow is written.

Context: zban and nx-rust keep the shell of their release workflows inline in the workflow file, with no tests. mara-23j2h9, filed by the audit after rc.1 and named in the kickoff prompt of the rc.2 slice, moved every step of `.github/workflows/release.yml` that did more than run a target or an action into scripts under `.github/scripts/`, with tests in `packages/mara-npm/tests/` that run the real scripts against loopback stubs of npm's registry and GitHub's API (`mara-npm:test`, part of `pnpm run checks`). The reason: Publish, Registry check and Tag run in no dry run, so before this their first execution was the publication itself, and the audit of the move found exit statuses that the inline shell discarded (a failed `jq` read as an empty list). `DES-RELEASE-WORKFLOW` and `ADR-DISPATCHED-RELEASE` (docs/distribution.mara.md, docs/decisions.mara.md) record it.

1. Keep the scripts and their tests (recommended; provisional until answered, and what `main` carries once mara-23j2h9 is merged).
2. Move the shell back inline, as in the references, and drop the tests. Undo: one issue that inlines the scripts; the fixes found by the tests stay.

- [ ] The maintainer has answered
