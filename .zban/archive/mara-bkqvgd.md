---
title: "When does the rewrite reach main: before rc.1 is published?"
status: done
type: task
tags:
  - maintainer
  - question
parent: mara-c8scn7
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T18:57:11Z
---

Question for the maintainer. `origin/main` is the 0.3.0 release commit (`29eec86`); the rewrite is on `chore/clear-worktree-for-rewrite`. GitHub starts a `workflow_dispatch` workflow only if its file is on the default branch, zban's release workflow accepts only `main` or `release/*`, and a coverage baseline or a CI run "on main" has nothing to run on while the rewrite is elsewhere.

Options:

1. Recommended: fast-forward `main` to the integration branch when the release workflow's dry run passes on the integration branch, then publish rc.1 from `main` (`git push origin chore/clear-worktree-for-rewrite:main`, as AGENTS.md says; the maintainer's push). Until then CI runs on pushes to the integration branch.
2. Make the integration branch the default branch on GitHub until 0.4.0, and publish from it.
3. Put only the workflow files on `main` and dispatch them for the integration branch.

Nothing waits on the answer before the release issue of the epic is done.

- [x] Answered

## Answer

The maintainer, 2026-11-22: "We will fast forward and actually proceed alpha6 with pr-based flow". Option 1: `main` is fast-forwarded to the integration branch by the maintainer, and the slice after rc.1 integrates by pull requests. The orchestrator reports when the release workflow's dry run has passed on the integration branch; the push is the maintainer's.
