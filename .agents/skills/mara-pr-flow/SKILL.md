---
name: mara-pr-flow
description: Publish Mara issue work to GitHub and manage its pull request when the user intends to push, open, or update that work. Coordinate a Luna PR subagent with the main implementation thread.
---

# Mara pull request flow

Use this skill with the repository's `AGENTS.md`. The main thread owns code,
verification, commits, and decisions about review findings. A `gpt-6-luna`
subagent owns PR publication, GitHub conversation operations, and review
monitoring. Keep the main turn active while the short review loop runs.

## Handoff from the main thread

Finish the authorized change and its verification before publication. Give the
subagent the Linear issue, intended branch, exact commit SHA, Mara IDs,
verification evidence, known limitations, and any existing PR URL. Explicitly
tell it which commit is ready to push. Delegate with model `gpt-6-luna`, a
bounded recent-turn fork, and an explicit handoff message that points to this
skill. The subagent must not edit source, make commits, amend, rebase,
force-push, or merge.

The subagent checks the branch and commit against the handoff before pushing.
It reuses an existing issue PR or opens one against `main`, following the
repository PR rules and template. Attach the PR to the Codex task and record
only verified evidence in Linear. Report the PR URL and published head SHA to
the main thread.

## Watch the current head

Poll GitHub about every 60 seconds for the PR head, CI, Codex review summary,
review submissions, and unresolved review threads. The Codex summary comment
contains `<!-- codex-pull-request-review-summary -->`; it can be edited in
place, so read its current body rather than looking only for new comments.
Treat 👀 as review in progress, and 👍 or a completed summary with no findings
as completion only when the reported reviewed commit matches the current PR
head. Check findings as well as the reaction; silence alone is not a pass.

The repository has triggered Codex review on new commits. Observe that trigger
before requesting another review. If no review starts within five minutes,
request `@codex review` once for that head. If review has not finished after
15 minutes, report the pending state and evidence to the main thread instead
of claiming a clean review.

Send each new finding to the main thread with its URL, reviewed SHA, and
conversation identifier. Deduplicate top-level and inline copies of one
finding. The main thread assesses it under `AGENTS.md` and either fixes and
verifies it, accepts backlog work, or supplies a reasoned reply. Do not
independently expand implementation scope.

After the main thread supplies a new verified commit, check its SHA and push
it to the same branch without rewriting history. Post the main thread's
supported reply and resolve only the specified inline review threads after
the fix or disposition is visible on the PR. Top-level PR comments can be
answered but have no resolvable thread. Create or link a Linear Backlog issue
only for an outcome the main thread accepts as backlog work. Resume monitoring
for the new head; an earlier clean review never clears a later commit.

Report that the PR is ready for merge only when the current head's review has
completed, required CI passes, and no current-scope or critical finding remains
unaddressed. Include any explicitly accepted backlog or dismissed findings in
that report. The main thread confirms acceptance criteria and merge readiness.
Do not merge unless separately instructed. Stop monitoring when the PR is
closed or the main thread ends the review loop.
