---
name: mara-pr-flow
description: Main-thread handoff for publishing or continuing a Mara issue pull request when the user expresses intent to send the work to GitHub.
---

# Mara pull request flow

This skill is for the main thread. The project `mara_pr_manager` agent owns
publication, review monitoring, GitHub conversations, and PR status. Its
instructions live in `.codex/agents/mara_pr_manager.toml`; do not ask it to use
this skill. Keep the main turn active during the review loop.

1. Complete and verify the authorized work, then commit it. Give the PR manager
   the Linear issue, branch, exact ready-to-push SHA, relevant Mara IDs,
   verification evidence, known limitations, and existing PR URL, if any.
   Select the project `mara_pr_manager` agent when the client supports custom
   agent selection. Otherwise, spawn a `gpt-6-luna` subagent with no forked
   turns; pass the handoff and the agent file's `developer_instructions` in its
   task, and explicitly forbid it from using this skill. Do not delegate
   product decisions or source edits.
2. When the agent reports a PR URL, attach that PR to this task. Review each
   finding it reports against `AGENTS.md` and the applicable Mara contract.
   Make and verify accepted fixes in the main thread, or decide on a supported
   reply, dismissal, or backlog outcome. Tell the agent the exact new commit
   SHA and the intended disposition of each finding. The agent handles the
   push, PR replies, and specified conversation resolutions.
3. Continue the same agent and PR for subsequent reviewed heads. Treat its
   merge-ready report as evidence to verify against the issue acceptance
   criteria. Merge only when separately instructed.
