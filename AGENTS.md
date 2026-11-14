# AGENTS.md

## Project state

- v0.4.0 is a rewrite from scratch. `docs/` specifies it and zban holds the work.
- The previous implementation is commit `d7f0e73` (branch `plan-execution-architecture`). Read it with `git show d7f0e73:<path>`. Carry over understanding, never text, and never modify it.
- The installed `mara` CLI, MCP tools and `mara` skill implement the old `:::mara` format. Never author with them.

## Writing rules

For every document in this repository, this file included:

- Keep a sentence only if it changes what someone builds, tests or decides.
- State each fact once; link to it everywhere else.
- No introductions, summaries, "this document describes…", or restated headings.
- Requirement: one testable obligation in 1–3 sentences. Design: bullets and tables. Rationale only in Decision items: the choice and rejected alternatives, one line each.
- Size limits: README ≤ 60 lines, AGENTS.md ≤ 120 lines, any docs page ≤ 200 lines (split it otherwise).

## Mara documents

- Engineering knowledge lives in `docs/*.mara.md`, listed in [docs/index.mara.md](docs/index.mara.md). README, this file and zban issues link to it and never restate it.
- Syntax: [item format](docs/format.mara.md) and [identity and links](docs/identity.mara.md). Types and relations: [engineering vocabulary](docs/vocabulary.mara.md).
- Layout follows the `mara-docs-layout` skill; the 200-line limit above replaces its 500-line signal.
- Make an item only for knowledge that needs its own identity, relations or verification. Everything else is narrative.
- An undecided matter is a plain bullet under `## Open questions` on its topic page, never an item. Settling it adds or changes an item and removes the bullet.
- `Accepted` means decided, not built. `Draft` marks a proposal or a provisional number.
- Add only relations that carry meaning. Never add an item or a link just to complete a chain.
- Update the affected items in the same change as the code.
- A requirement is Accepted together with its Verification item. The rewrite is the exception: until 0.4.0 ships, a slice's Verification items are written before its work starts, by the issue that blocks the slice's epic.
- Ask before deciding anything that changes user-visible behaviour, a persisted format, data safety or scope. Small reversible choices need no question. Until 0.4.0 is published, [Deciding during the rewrite](#deciding-during-the-rewrite) replaces this rule.

### Writing items by hand

Until Mara can write the new format:

- Generate each MID with:

  ```sh
  python3 -c "import os,time;n=(int(time.time()*1000)<<80)|int.from_bytes(os.urandom(10),'big');a='0123456789ABCDEFGHJKMNPQRSTVWXYZ';print(''.join(a[(n>>(5*i))&31] for i in range(25,-1,-1)))"
  ```

- Never copy or reuse a MID.
- Put a `---` before every item and a blank line on both sides of every `---`.
- When you move an item or rename an ID, update the path and text of every link to it.

## Work tracking

- Work lives in zban issues under `.zban/`. `zban prime` prints the workflow; change issues only with `zban`.
- One epic per release slice, `0.4.0-alpha.1` to `0.4.0`. Each issue has one outcome and names the Mara IDs it implements.
- A decision made while working goes into a Mara document, not only into an issue.

## Deciding during the rewrite

One orchestrator starts a worker for each ready issue and integrates its branch.

- A worker decides what the docs leave open: follow the docs, then 0.3.0 where the capability is kept, then the simplest option. It writes the decision into the item it belongs to and lists it under `## Decisions made` when it closes the issue.
- A worker never decides alone to contradict an Accepted item, to add or drop a capability, or to loosen a requirement on data safety or security. It stops work on the issue and reports the question, with its recommended answer, to the orchestrator.
- The orchestrator files such a question as an issue tagged `maintainer` that blocks the original issue, sets that issue back to `todo` and continues with other ready issues. It stops only when none is ready.
- No worker starts an issue tagged `maintainer` or `deferred`.
- When a slice's epic closes, the orchestrator appends the `## Decisions made` lists of its issues to the slice's review issue. That issue blocks the release, not the next slice. An overturned decision becomes a new issue.
- A worker changes nothing outside its worktree and its temporary directories.

## Engineering

- Rust. Commit `Cargo.lock`, pin the toolchain, keep one workspace version.
- The engine crate does no I/O and builds for `wasm32` ([engine](docs/engine.mara.md)).
- Every new operation ships with its work-counter and scaling tests ([verification](docs/verification.mara.md)).
- Take the shortest path to a working end-to-end slice. Add no abstraction, dependency or framework for a hypothetical need.
- Build no test framework, runner, generator, linter or build system that `docs/` does not specify, unless the maintainer approves it.
- A feature is done when the real workflow runs end to end. Mocks and isolated tests do not show that.
- Tests run in disposable projects in temporary directories, with their own configuration and Git state. They never touch the checkout, global configuration or a daemon they did not start.
- A failure preserves user data and reports a diagnostic the user can act on.
- Every persisted or public format has its own version, starting at 1.
- Stop when the issue's outcome works and its checks pass. File follow-up work in zban.

## Review

- Review against the issue, the Mara items it names and the real workflow.
- Report only defects with a concrete impact. Formatting and lint belong to the automated checks, not to review.

## Commits and releases

- Conventional Commits without scopes: `<type>: <description>`, with `!` for a breaking change. Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.
- Commit a completed, verified change as one logical commit unless told otherwise. Stage only intended files, and include the zban issue files the change touches.
- Never amend, rewrite history or push without authorization.
- Semantic Versioning. Tag releasable slices with annotated `vX.Y.Z[-prerelease]` tags.

### Until the rewrite is on `main`

- The rewrite integrates on `chore/clear-worktree-for-rewrite`.
- One branch and worktree per zban issue, cut from the integration branch's tip. The orchestrator first sets the issue `in-progress` on the integration branch, so no two workers take the same issue.
- Before its branch is integrated, a worker rebases it onto the integration branch's tip and reruns its checks. Rebasing its own unmerged branch is allowed.
- A worker never merges into the integration branch. The orchestrator integrates one branch at a time: squash merge into one commit, then delete the branch. Follow-up work starts from the new tip.
- No CI exists until the last slice (`mara-4dr2p2`), and no workflow file is added before it. Run the checks locally before handing over a branch and again before integrating it.
- The integration branch reaches `origin/main` only by fast-forward: `git push origin chore/clear-worktree-for-rewrite:main`, with branch protection lifted for that push. Never squash it and never use GitHub's "Rebase and merge": both change commit SHAs, and `d7f0e73` and the alpha tags must stay on `main`.
