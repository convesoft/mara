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
- 2026-11-22: the workspace files are copied from Nx's TypeScript preset, generated in a temporary directory, so that they agree with what Nx generates and hold what the alpha.6 packages need; the generator never runs inside the checkout.
- 2026-11-22: of the generator's agent files, the repository takes the Nx skills it will use and the managed Nx block of `AGENTS.md`; the size rule of `AGENTS.md` does not count that block. The linter and the formatter of the JS side are oxlint and oxfmt.
- 2026-11-22: a last issue of the epic (mara-mrmg89) switches to the published package.
- 2026-11-22 (mara-tt31bp): loaded from its TypeScript source, nx-rust runs no `test` and restores no `build` from the cache, because it starts two helper scripts that exist only in a build. The repository waits for the npm release and takes the plugin from there (chosen over linking to a build of the plugin and over fixing the plugin for source loading). This replaces the decision to load from source; mara-a4ga3m does mara-mrmg89 in the same change, and nothing of the epic or of alpha.6 starts until the package is published.
- 2026-11-22: the maintainer published `@convesoft/nx-rust@1.0.0-rc.1` (dist-tag `next`; `latest` is still the `0.0.0` placeholder) as the release to use. The repository pins that exact version (integrated with mara-a4ga3m and mara-mrmg89).

## Decided by the orchestrator

One line per decision made on the maintainer's behalf: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-21, provisional: the Nx targets are added beside the cargo commands, which stay in AGENTS.md as what the targets run and as the fallback; the orchestrator's gate runs the checks both ways until a run has shown the same results, then through Nx. Rests on nx-rust being unreleased and this repository being the first real one it runs on. Undo: remove the cargo commands from "Local checks".
- 2026-11-21, provisional: worktrees do not share an Nx cache, so the orchestrator's gate executes its checks itself and restores nothing a worker produced. Rests on the gate being an independent check (the orchestrator's rules). Undo: one cache directory for all worktrees.

- 2026-11-22: `lint` denies warnings through `CARGO_ENCODED_RUSTFLAGS=-Dwarnings` in `.env.lint`, with two accepted limits: a caller's `RUSTFLAGS` does not reach `lint`, and a caller's own `CARGO_ENCODED_RUSTFLAGS` replaces the denial. Rests on the plugin always passing `--message-format=json`, under which cargo ignores `build.warnings`, and on the cargo clippy command staying beside the target; ADR-NX-RUST-TASKS states both limits. Undo: a plugin option for extra clippy arguments, once nx-rust has one.
- 2026-11-22: `pnpm-workspace.yaml` exempts exactly `@convesoft/nx-rust@1.0.0-rc.1` from pnpm's minimum release age, without which the frozen install fails while the release is new. Rests on the maintainer having published that version. Undo: remove the entry once the release is older than pnpm's limit, or when the pin moves to `1.0.0` (ADR-NX-RUST-TASKS says so).
- 2026-11-22: Nx 24.0.6 keeps the cache of every checkout of one repository in one per-user folder by default, so `nx.json` sets the cache directory inside the checkout. Rests on the provisional decision above that worktrees share no cache, which only the maintainer changes. Undo: remove `cacheDirectory`.
- 2026-11-22: three findings of the review of mara-a4ga3m are not fixed here, because the plugin decides them and this repository uses none of the settings: an edit to the file `CLIPPY_CONF_DIR` selects, to a per-user Rustfmt configuration, or to Cargo configuration outside the repository restores the old result. Reported to the maintainer for nx-rust (archived mara-tt31bp); ADR-NX-RUST-TASKS says in one sentence that configuration outside the repository is not in the keys. Undo: declare them as `runtimeInputs` here.
- 2026-11-22: tools on `PATH` that the tests start (`tar`, `cc`, `mkfifo`, `git`) and local Git archive attributes are not in the key of `mara:test`; backlog mara-5c83v4. Rests on the finding being exotic on this project's machines.
