# Delivery conventions

## Ownership

Git-tracked Mara documents own durable product meaning. Work items own delivery scope, acceptance criteria, sequencing, and progress. GitHub owns PRs, reviews, CI, and merge evidence. Delivery scopes are a repository workflow, not a new Mara flavour or a change to Work's generic lifecycle.

## Implementation delivery

Both delivery modes are supported. Select the mode before starting new work; keep existing branches unless explicitly changing their placement.

### Individual issue

Start the item branch from current `main` using `<work-id>-<short-title>` (an unambiguous `w-` prefix). Open one bounded PR against `main` and squash merge it.

### Bounded local scope

1. Use one manual Work item to own a bounded version or milestone scope. Record the repository, intended outcome, included canonical Work IDs, relevant Mara IDs, integration order, and final acceptance checks. Reuse the release-preparation owner when it already owns this scope; a version name is not a release or publication approval.
2. Start a short-lived scope branch from current `main` using `<work-id>-<short-title>` (an unambiguous `w-` prefix) with the scope owner's ID. Start each included issue branch from the current scope tip using the same convention with that issue's ID. Do not introduce long-lived development or release branches.
3. Implement, verify, and locally review each issue against its scope base before integration. Follow the repository's review rules; run the independent Pi review loop only when explicitly required. Squash integration is not review or CI.
4. With a clean scope checkout, run `git merge --squash <issue-branch>`, inspect the staged result, and commit one logical Conventional Commit for that issue. Identify its canonical Work ID in the commit body or footer. This creates no merge commit and does not record merge ancestry. Verify the integrated result before starting dependent work; after integration, branch subsequent work from the new scope tip rather than reusing the old issue branch.
5. Keep issue acceptance evidence in its item and record the integrated commit and remaining work in the scope owner. Local integration does not close an implementation item.
6. Run the final combined acceptance checks on the exact scope head. Open one scope PR against `main`, identifying the owner and all included items, relevant Mara IDs, actual verification evidence, and limitations. Every retained commit must be independently useful and follow the commit convention. Rebase merge the scope PR to preserve per-issue commits in linear `main` history; do not squash the entire scope or use a merge commit.

If `main` advances, verify the updated integration result before merging. Updating or rewriting an existing branch requires explicit authorization; this workflow does not authorize incidental rebases or force-pushes. A changed PR head requires fresh applicable checks and review. GitHub must allow rebase merging for scope PRs; unavailable merge methods are a blocker, not permission to silently choose another method.

### Evidence and completion

Work IDs remain stable across Git history rewriting. Local squashing and GitHub rebase merging change commit identities. Evidence records the actual tested revision and scope; do not relabel old evidence as testing a new SHA. Record the final `main` integration revision and run the applicable final-`main` checks. Release verification still targets one captured `main` revision.

Keep implementation items open through local integration, PR review, and fixes. Close each only after its acceptance passes and its change reaches `main`. Close the manual scope owner only after its own combined acceptance and integration gates pass. Aggregate completion remains derived from children; a manual owner is not a derived aggregate.

Pushes, PR merges, branch/history rewrites, and release publication require their respective authorization. Adoption of this workflow does not itself authorize them.

## Release preparation

One manual Work item owns release preparation and publication follow-through. It may use either delivery mode; in scope mode, keep release preparation independently identifiable and reviewed in the scope PR. A merged scope is not publication approval. Follow [the release contract](release.mara.md) for version selection, generated changelog, captured-main verification, tags, and protected publication.
