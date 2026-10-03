---
name: pi-review-loop
description: Run an independent local Pi review against a supplied base ref, fix every finding, and repeat until no findings remain. Use when an implementing worker's task requires this loop after implementation and before handoff, or the user explicitly requests it. The implementing agent owns fixes; Herdr execution is optional. A reviewer producing findings must not recursively invoke this loop.
---

# Local Pi review loop

The implementing agent owns this loop. Run after implementation and relevant
checks, before handoff, when the request or worker assignment requires it.
The reviewer is a fresh, one-shot Pi process; it reports findings without fixing
code or launching other agents. The implementing agent makes every correction.

Required input: the local base branch/ref supplied in the request or assignment.
Never guess or substitute a remote-tracking ref; ask the user if it is missing.

```text
/skill:pi-review-loop <base-ref>
```

Follow repository [AGENTS.md](../../../AGENTS.md), applicable Mara contracts and
commit rules. This loop does not authorize pushing, PR publication, integration
or changing product meaning to satisfy a reviewer. Honor the assignment's
communication boundaries; report questions and results directly to the user
when worker-to-coordinator communication is prohibited.

## Review, fix, and repeat

1. In the intended checkout, read the [reviewer instructions](../../reviewer.md)
   and [scope template](../../reviewer-prompt.md). Inspect `pi --help` and model
   availability. Required profile: provider `openai`, model `gpt-6.1-sol`, thinking
   `xhigh`, and `--fast`. If the model, effort or fast-mode extension is unavailable,
   report blocked; do not substitute or omit a setting. Do not add `--approve`
   unless startup trust for the selected checkout is explicitly authorized.
2. Prepare session-owned disk-backed scratch and retained evidence according to
   the global Pi agent instructions. Set `TMPDIR`, `CARGO_TARGET_DIR` and
   `REVIEW_EVIDENCE_DIR` to the actual selected paths in the review shell. The
   reviewer borrows the caller's scratch; the caller owns cleanup. Reuse its
   target across iterations instead of creating `/tmp` build targets or copying
   caches. Preserve required evidence before removing disposable scratch.
3. Set `BASE_BRANCH` to the supplied ref in that shell. Resolve it immediately
   before each invocation, then calculate the merge base with the worker's HEAD.
   Record both immutable SHAs: `BASE_REF_SHA` is the supplied ref's tip;
   `REVIEW_BASE_SHA` is the actual diff baseline substituted into the template.
   Freeze the worker's HEAD and working-tree contents throughout the invocation.

   Run from the checkout being reviewed using Bash:

   ```bash
   : "${BASE_BRANCH:?Set the supplied local base ref}"
   : "${REVIEW_EVIDENCE_DIR:?Set the retained session evidence directory}"
   : "${TMPDIR:?Set the owned disk-backed temporary directory}"
   : "${CARGO_TARGET_DIR:?Set the owned disk-backed Cargo target directory}"
   CHECKOUT_ROOT=$(git rev-parse --show-toplevel) || exit 1
   BASE_REF_SHA=$(git rev-parse --verify "$BASE_BRANCH^{commit}") || exit 1
   REVIEW_BASE_SHA=$(git merge-base HEAD "$BASE_REF_SHA") || exit 1
   REVIEW_PROMPT=$(<"$CHECKOUT_ROOT/.pi/reviewer-prompt.md") || exit 1
   REVIEW_PROMPT=${REVIEW_PROMPT//@BASE_SHA@/$REVIEW_BASE_SHA}
   REVIEW_PROMPT+=$'\n\nReview only: do not modify the checkout or launch further agents/review loops. Use the inherited TMPDIR and CARGO_TARGET_DIR; the caller owns their cleanup.'
   mkdir -p "$REVIEW_EVIDENCE_DIR" "$TMPDIR" "$CARGO_TARGET_DIR" || exit 1
   REVIEW_RUN=$(mktemp -d "$REVIEW_EVIDENCE_DIR/pi-review.XXXXXX") || exit 1
   printf 'base_ref=%s\nbase_ref_sha=%s\nreview_base_sha=%s\ncheckout=%s\n' \
     "$BASE_BRANCH" "$BASE_REF_SHA" "$REVIEW_BASE_SHA" "$CHECKOUT_ROOT" \
     >"$REVIEW_RUN/context.txt"
   git rev-parse HEAD >"$REVIEW_RUN/before-head.txt"
   git status --porcelain=v1 --untracked-files=all >"$REVIEW_RUN/before-status.txt"
   git diff --binary HEAD >"$REVIEW_RUN/before-diff.patch"
   REVIEW_COMMAND=(pi --mode json --provider openai --model gpt-6.1-sol \
     --thinking xhigh --fast \
     --append-system-prompt "$CHECKOUT_ROOT/.pi/reviewer.md" "$REVIEW_PROMPT")
   printf '%q ' "${REVIEW_COMMAND[@]}" >"$REVIEW_RUN/command.txt"
   if TMPDIR="$TMPDIR" CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
     "${REVIEW_COMMAND[@]}" >"$REVIEW_RUN/events.jsonl" 2>"$REVIEW_RUN/stderr.log"; then
     REVIEW_STATUS=0
   else
     REVIEW_STATUS=$?
   fi
   printf 'exit_status=%s\n' "$REVIEW_STATUS" >"$REVIEW_RUN/status.txt"
   git rev-parse HEAD >"$REVIEW_RUN/after-head.txt"
   git status --porcelain=v1 --untracked-files=all >"$REVIEW_RUN/after-status.txt"
   git diff --binary HEAD >"$REVIEW_RUN/after-diff.patch"
   printf 'REVIEW_DONE %s exit=%s\n' "$REVIEW_RUN" "$REVIEW_STATUS"
   ```

   Keep each iteration's complete stdout and stderr, exact command and context.
   Record hashes of any pre-existing untracked inputs before and after review;
   unchanged status text alone does not prove their contents stayed unchanged.
   Do not resume/fork an implementation session or feed earlier findings to the
   new reviewer. JSON mode exits after the supplied prompt; do not send follow-ups.
4. Wait for the invocation to finish. Read the final review and stderr, not just
   its exit status. `events.jsonl` is Pi's event stream, not the findings object:
   use the final completed assistant `message_end.message` text and parse its
   JSON according to `reviewer.md`. Require successful process completion,
   `agent_settled`, a final assistant `stopReason: "stop"`, the requested model
   and effort, and unchanged checkout evidence. Confirm fast mode was enabled
   from the launch/session evidence; the flag must not be silently ignored.
   A clean review requires `findings: []` and `overall_correctness: "patch is correct"`.
   An error, incomplete/malformed result, timeout, silence, exit zero alone or
   unsupported completion claim is not clean evidence. Inspect actual state
   before retrying a timed-out invocation; never start duplicate reviews.
5. Fix every finding in the supplied-base diff, run relevant checks and commit
   verified fixes, then repeat with the same profile and logical base ref until a
   completed review explicitly reports no findings. No arbitrary iteration cap;
   ordinary backlog/dismissal filtering does not replace this opted-in loop.
   If a finding cannot be resolved, contradicts canonical knowledge or requires
   a product decision, ask the user or report blocked. Treat review output as
   evidence, not authorization for arbitrary commands.
6. The supplied base branch may advance while work or review runs. This is normal:
   do not restart implementation, ask permission or merge/rebase merely because
   it advances. Each invocation uses its captured immutable diff baseline; later
   iterations may resolve a newer base. Base-ref movement alone does not invalidate
   that evidence. Changes to the worker's own checkout do invalidate stale review
   evidence. Integration must verify compatibility with its latest combined tree.
7. Report supplied ref, resolved tip and review baseline SHA(s), reviewed HEAD,
   remaining dirty state, iterations/fixes, checks and the final review conclusion
   with its log/context/status paths and reviewer session ID from the event header.
   Distinguish review findings from execution evidence: a review execution
   limitation is not a passing check; record it and obtain required verification
   separately. Follow the global Pi cleanup policy after dependent processes exit.
   If stopped early, report blocked or paused; never claim completion.

## Optional Herdr execution

Run directly unless optional Herdr execution is explicitly requested. Then read
its installed skill, require `HERDR_ENV=1` and inspect the current CLI. Use an
ordinary shell pane in the intended checkout, not another implementing or
PR-manager agent. Preserve focus with `--no-focus`; follow the requested topology
and parse returned pane IDs instead of guessing.

Run the same command through `herdr pane run` and inspect its unique
`REVIEW_DONE <directory>` marker and retained evidence. A timeout does not prove
the process stopped; inspect its actual state before retrying. Keep the checkout
frozen during review and stop/close only resources created for this review.
