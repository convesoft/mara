# AGENTS.md

## Project state

- v0.4.0 is a rewrite from scratch. `docs/` specifies it and zban holds the work.
- The previous implementation is commit `d7f0e73` (branch `plan-execution-architecture`). Read it with `git show d7f0e73:<path>`. Carry over understanding, never text, and never modify it.
- The installed `mara` CLI, MCP tools and `mara` skill implement the old `:::mara` format. Never use them, also after the rewrite can author: use the binary built in your worktree until 0.4.0 is installed.

## Writing rules

For every document in this repository, this file included:

- Keep a sentence only if it changes what someone builds, tests or decides.
- State each fact once; link to it everywhere else.
- No introductions, summaries, "this document describes…", or restated headings.
- Requirement: one testable obligation in 1–3 sentences. Design: bullets and tables. Rationale only in Decision items: the choice and rejected alternatives, one line each.
- Size limits: README ≤ 60 lines, AGENTS.md ≤ 120 lines without the managed Nx block.

## Mara documents

- Engineering knowledge lives in `docs/**/*.mara.md`, listed in [docs/index.mara.md](docs/index.mara.md). README, this file and zban issues link to it and never restate it.
- Syntax: [item format](docs/format.mara.md) and [identity and links](docs/identity.mara.md). Types and relations: [engineering vocabulary](docs/template/vocabulary.mara.md).
- [.mara/config.toml](.mara/config.toml) makes `docs/` a Mara project. Its schema, [.mara/schema.toml](.mara/schema.toml), is the engineering vocabulary and changes only with that page. The [local checks](#local-checks) validate the project with the worktree's binary.
- Layout follows the `mara-docs-layout` skill.
- Make an item only for knowledge that needs its own identity, relations or verification. Everything else is narrative.
- An undecided matter is a plain bullet under `## Open questions` on its topic page, never an item. Settling it adds or changes an item and removes the bullet.
- `Accepted` means decided, not built. `Draft` marks a proposal or a provisional number.
- Add only relations that carry meaning. Never add an item or a link just to complete a chain.
- Update the affected items in the same change as the code.
- Code that implements an Accepted requirement carries `@mara implements <ID>` ([DES-CODE-MARKERS](docs/code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) on each function or type that carries out a distinct obligation of it, for the hosts that exist; a requirement that no single place or only `.mara/rules/*.dl` implements carries none. Markers change with the code. Every `.rs` file under `crates/` is scanned at the grammar level, so only comments hold markers.
- Read and change `docs/` with the binary built in your worktree, in direct mode, as `cargo run --quiet --bin mara -- --no-daemon <operation>`. When your context lacks its guide, headed `# Mara workflow`, run `cargo run --quiet --bin mara -- prime` first.
- A requirement is Accepted together with its Verification item. The rewrite is the exception: until 0.4.0 ships, a slice's Verification items are written before its work starts, by the issue that blocks the slice's epic.
- Ask before deciding anything that changes user-visible behaviour, a persisted format, data safety or scope. Small reversible choices need no question. Until 0.4.0 is published, [Deciding during the rewrite](#deciding-during-the-rewrite) replaces this rule.

## Work tracking

- Work lives in zban issues under `.zban/`. `zban prime` prints the workflow; change issues only with `zban`.
- One epic per release slice; [DES-PACKAGES](docs/distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) names the slices up to `0.4.0`. Each issue has one outcome and names the Mara IDs it implements.
- A decision made while working goes into a Mara document, not only into an issue.

## Deciding during the rewrite

One orchestrator starts a worker for each ready issue and integrates its branch.

- A worker decides what the docs leave open: follow the docs, then 0.3.0 where the capability is kept, then the simplest option. It writes the decision into the item it belongs to and lists it under `## Decisions made` when it closes the issue.
- A worker never decides alone to contradict an Accepted item, to add or drop a capability, or to loosen a requirement on data safety or security. It stops work on the issue and reports the question, with its recommended answer, to the orchestrator.
- The orchestrator files such a question as an issue tagged `maintainer` that blocks the original issue, sets that issue back to `todo` and continues with other ready issues. It stops only when none is ready.
- No worker starts an issue tagged `maintainer` or `deferred`.
- When a slice's epic closes, the orchestrator appends the `## Decisions made` lists of its issues to the slice's review issue. That issue blocks the release, not the next slice. An overturned decision becomes a new issue.
- A worker changes nothing outside its worktree and its temporary directories except for remote operations authorized by the integration rules below.

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

## Local checks

The local checks are `pnpm run checks`; while working, `pnpm run checks:fast` leaves out `mara:test:performance` and `mara-engine:build:wasm32`. Each check is an Nx target, and [ADR-NX-RUST-TASKS](docs/decisions.mara.md#01M8094G2DV9JZ35C8VRMFPWYV) names the command each runs. Run them from the repository root; each must exit 0.

- Once in a fresh worktree: `pnpm install --frozen-lockfile`. The checks build Soufflé when it is missing.
- Once on each machine: `cargo run --quiet --bin mara -- --no-daemon code add rust`, because `mara:validate` needs this repository's pinned grammar in the per-user cache ([DES-GRAMMAR-ASSETS](docs/code/index.mara.md#01M7BF6GQC7MW6ZSA9NMEQRQHM)).
- `rustup` installs the toolchain, components and target that `rust-toolchain.toml` pins.
- `mara-engine:build:wasm32` is an architecture check of [VER-HOST-COMPARISON](docs/verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9); `mara:test:performance` times a release build for [VER-SCALING](docs/verification.mara.md#01M7BF6HGRNQ5SJVNP89G1CCBW).
- On this machine run Nx with `NX_DAEMON=false NX_USE_LOCAL=true` (no daemon, the worktree's own Nx) under `berth --`, and `pnpm run checks` under `berth --exclusive --`, because the performance test needs the machine to itself.
- CI runs the local checks on the pushes and pull requests named by [DES-CI-WORKFLOW](docs/distribution.mara.md#01M82C8WTKYXH8S92ZJQ4FX73R). CI replaces no local run.

## Review

- Review against the issue, the Mara items it names and the real workflow.
- Report only defects with a concrete impact. Formatting and lint belong to the automated checks, not to review.

## Commits and releases

- Conventional Commits without scopes: `<type>: <description>`, with `!` for a breaking change. Types: `feat`, `fix`, `docs`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`.
- Commit a completed, verified change as one logical commit unless told otherwise. Stage only intended files, and include the zban issue files the change touches.
- Never amend, rewrite history or push without authorization.
- Semantic Versioning. The alpha slices carry annotated `v0.4.0-alpha.N` tags. From `0.4.0-rc.1` on, only the release workflow tags a version, when it publishes it ([DES-RELEASE-WORKFLOW](docs/distribution.mara.md#01M82C8WTK3M2A69XWRGNNQ0SF)); nobody tags by hand, and an agent starts that workflow only as a dry run.

### Pull requests

- One branch and worktree per zban issue, cut from `origin/main`. The worker sets its issue `in-progress` in the first commit of its branch.
- Before it opens its pull request, the worker merges `origin/main` into its branch, runs the [local checks](#local-checks) and pushes the branch. The worker is authorized to push its issue branch and open its pull request to `main`.
- The pull request follows [the template](.github/pull_request_template.md). Give it a title that follows the commit rules above. Write each paragraph of the description on one line and complete the `Refs:` footer with the issue ID.
- A worker never merges its own pull request, never force-pushes and never pushes to `main`.
- Only the orchestrator squash-merges pull requests to `main`, one at a time after the required checks pass. Use the pull request title as the squash commit subject and the description as its body. The orchestrator's tracker-only changes go through a pull request of their own that changes only `.zban/`.
- Nobody rewrites the history of `main`: `d7f0e73` and the alpha tags stay on it.

### Until `main` is fast-forwarded

[Pull requests](#pull-requests) apply from the fast-forward of `main` on. Until then:

- The rewrite integrates on `chore/clear-worktree-for-rewrite`. For each zban issue, the orchestrator first sets it `in-progress` on the integration branch. The worker then creates one branch from its tip in a dedicated worktree.
- A worker rebases its own unmerged branch onto, or merges into it, the integration branch's tip, reruns the local checks and hands over the branch unpushed. This rule authorizes the rebase. Only the orchestrator integrates into the integration branch. It reruns the local checks, squash-merges one branch at a time and deletes each merged branch.
- The maintainer fast-forwards `origin/main` to the integration branch: `git push origin chore/clear-worktree-for-rewrite:main`, with branch protection lifted for that push. Never squash it and never use GitHub's "Rebase and merge": both change commit SHAs.

## Nx

The block below names an Nx MCP server and `nx_docs`; neither is configured here.

<!-- nx configuration start-->
<!-- Leave the start & end comments to automatically receive updates. -->

# General Guidelines for working with Nx

- For navigating/exploring the workspace, invoke the `nx-workspace` skill first - it has patterns for querying projects, targets, and dependencies
- When running tasks (for example build, lint, test, e2e, etc.), always prefer running the task through `nx` (i.e. `nx run`, `nx run-many`, `nx affected`) instead of using the underlying tooling directly
- Prefix nx commands with the workspace's package manager (e.g., `pnpm nx build`, `npm exec nx test`) - avoids using globally installed CLI
- You have access to the Nx MCP server and its tools, use them to help the user
- For Nx plugin best practices, check `node_modules/@nx/<plugin>/PLUGIN.md`. Not all plugins have this file - proceed without it if unavailable.
- NEVER guess CLI flags - always check nx_docs or `--help` first when unsure

## Scaffolding & Generators

- For scaffolding tasks (creating apps, libs, project structure, setup), ALWAYS invoke the `nx-generate` skill FIRST before exploring or calling MCP tools

## When to use nx_docs

- USE for: advanced config options, unfamiliar flags, migration guides, plugin configuration, edge cases
- DON'T USE for: basic generator syntax (`nx g @nx/react:app`), standard commands, things you already know
- The `nx-generate` skill handles generator discovery internally - don't call nx_docs just to look up generator syntax

<!-- nx configuration end-->
