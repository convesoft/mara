---
title: "The rewrite integrates by pull requests after the fast-forward of main: AGENTS.md, the template and the required checks"
status: todo
type: task
parent: mara-c8scn7
blocked_by:
  - mara-1avrq4
created_at: 2026-11-22T18:57:11Z
---

Goal: the repository's instructions describe integration by pull requests to `main`, so that the slice after rc.1 can run that way from its first issue.

Context: decided by the maintainer on 2026-11-22: "We will fast forward and actually proceed alpha6 with pr-based flow". AGENTS.md, "Until the rewrite is on `main`", describes the local squash flow on `chore/clear-worktree-for-rewrite` and says no CI exists. The zban and nx-rust repositories already work by pull requests (read-only references: their AGENTS.md and `.github/`).

- [ ] AGENTS.md: the section "Until the rewrite is on `main`" is replaced by the rules after the fast-forward: one branch per issue cut from `main`, a pull request per issue that the orchestrator squash-merges when the required checks pass, the issue set `in-progress` in the branch's first commit, who may merge, and that `d7f0e73` and the alpha tags stay on `main`. The fast-forward itself stays described until it has happened. Within the size limit
- [ ] A pull request template whose title and description become the squash commit under the repository's commit rules
- [ ] The names of the CI checks that a pull request must pass are listed in the closing summary for the maintainer, who sets branch protection
- [ ] "Deciding during the rewrite" and "Local checks" agree with the new flow (what a worker runs before it opens a pull request, what CI runs)

Not part of this issue: the fast-forward of `main`, branch protection and every other repository setting (the maintainer's), the orchestrator's own rules.
