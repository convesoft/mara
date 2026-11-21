---
title: Review the decisions made in the Nx workspace epic
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-1fab1q
created_at: 2026-11-21T23:23:39Z
---

Goal: the maintainer reviews what the orchestrator and the workers decided on their own in the Nx workspace epic (mara-1fab1q).

Context: AGENTS.md (Deciding during the rewrite). When the epic closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the maintainer

- 2026-11-21: the repository moves to Nx with `@convesoft/nx-rust` before the alpha.6 slice starts, as a small epic.
- 2026-11-21: until the package is on npm, the plugin is used from the maintainer's local nx-rust checkout, by path (chosen over a vendored tarball, a GitHub pre-release and waiting for npm), and on 2026-11-22: from its TypeScript source, without a build. It works on this machine only.
- 2026-11-21: what the plugin does not handle is a custom Nx target (one that calls `zban`, for example), and `package.json` scripts combine the usual checks.

## Decided by the orchestrator

One line per decision made on the maintainer's behalf: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-21, provisional: the Nx targets are added beside the cargo commands, which stay in AGENTS.md as what the targets run and as the fallback; the orchestrator's gate runs the checks both ways until a run has shown the same results, then through Nx. Rests on nx-rust being unreleased and this repository being the first real one it runs on. Undo: remove the cargo commands from "Local checks".
- 2026-11-21, provisional: worktrees do not share an Nx cache, so the orchestrator's gate executes its checks itself and restores nothing a worker produced. Rests on the gate being an independent check (the orchestrator's rules). Undo: one cache directory for all worktrees.
