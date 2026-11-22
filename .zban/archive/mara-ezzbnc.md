---
title: "The rewrite integrates by pull requests after the fast-forward of main: AGENTS.md, the template and the required checks"
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-1avrq4
created_at: 2026-11-22T18:57:11Z
closed_at: 2026-11-22T23:02:37Z
---

Goal: the repository's instructions describe integration by pull requests to `main`, so that the slice after rc.1 can run that way from its first issue.

Context: decided by the maintainer on 2026-11-22: "We will fast forward and actually proceed alpha6 with pr-based flow". AGENTS.md, "Until the rewrite is on `main`", describes the local squash flow on `chore/clear-worktree-for-rewrite` and names CI on pushes to that branch. The zban and nx-rust repositories already work by pull requests (read-only references: their AGENTS.md and `.github/`).

- [x] AGENTS.md: the section "Until the rewrite is on `main`" is replaced by the rules after the fast-forward: one branch per issue cut from `main`, a pull request per issue that the orchestrator squash-merges when the required checks pass, the issue set `in-progress` in the branch's first commit, who may merge, and that `d7f0e73` and the alpha tags stay on `main`. The fast-forward itself stays described until it has happened. Within the size limit
- [x] A pull request template whose title and description become the squash commit under the repository's commit rules
- [x] The names of the CI checks that a pull request must pass are listed in the closing summary for the maintainer, who sets branch protection
- [x] "Deciding during the rewrite" and "Local checks" agree with the new flow (what a worker runs before it opens a pull request, what CI runs)

Not part of this issue: the fast-forward of `main`, branch protection and every other repository setting (the maintainer's), the orchestrator's own rules.

## Summary of Changes

- AGENTS.md: "Until the rewrite is on `main`" became two sections under "Commits and releases": "Pull requests", the flow after the fast-forward (branch from `origin/main`, `in-progress` in the branch's first commit, merge `origin/main` and run the local checks before opening the pull request, a title under the commit rules and a `Refs:` footer, no self-merge, no force-push, no push to `main`, one squash merge at a time by the orchestrator after the required checks, with the title as subject and the description as body, tracker-only pull requests that change only `.zban/`, `main`'s history and `d7f0e73` and the alpha tags kept), and "Until `main` is fast-forwarded", the local squash flow on the integration branch (one branch and worktree per issue, the worker's rebase or merge of the tip, only the orchestrator integrates) and the fast-forward command. Deleting the second section leaves the first true on its own.
- "Deciding during the rewrite": a worker may do the remote operations that the integration rules authorize. "Local checks": CI runs the local checks on the events DES-CI-WORKFLOW names and replaces no local run; the line that names the commands is unchanged.
- `.github/pull_request_template.md`: a descriptive paragraph, Mara items and a `Refs: mara-` footer; no heading, no checklist. AGENTS.md is 104 lines without the Nx block; `CLAUDE.md` is still the link.
- Checks: `berth --exclusive -- pnpm run checks`, exit 0: `project validate` 0 errors, 0 warnings; 860 passed, 0 failed, 16 ignored.

## Required checks for branch protection on `main`

- From `.github/workflows/ci.yml` (workflow `CI`): `Checks (ubuntu-24.04)` and `Checks (ubuntu-24.04-arm)`.
- `release.yml` (not built yet): it runs on a pull request only when the pull request changes a release input (DES-RELEASE-WORKFLOW), and GitHub keeps a required check that never runs pending, which blocks the merge. So none of its jobs should be a required check while that path filter stands. If the maintainer wants the dry run to gate the pull requests that do run it, the job is `Dry run`, the last in its chain (Prepare, Checks, Pack dispatcher, Build, Dry run). `Dry run` is the final job of the pull-request dry run. To require it on every pull request to `main`, first update DES-RELEASE-WORKFLOW and the workflow so that it runs on every such pull request.

## Decisions made

- The flow after the fast-forward is a section of its own, and the flow before it is a separate section that only defers to it, so that removing the "before" part later is a deletion and nothing else.
- Each paragraph of the pull request description goes on one line, because GitHub wraps the squash commit body (zban's and nx-rust's rule).
- The template has no checklist and no HTML comments, because the description becomes the commit body verbatim. The zban issue goes in a `Refs:` footer, as zban and nx-rust do it.
- The CI bullet in "Local checks" links DES-CI-WORKFLOW for the events instead of listing branches, so it stays true after the fast-forward.
- The fast-forward is the maintainer's push (mara-bkqvgd).
