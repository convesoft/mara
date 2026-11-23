---
title: Release notes come from nx release changelog, as in zban and nx-rust
status: done
type: task
parent: mara-xzfeyj
created_at: 2026-11-23T13:14:28Z
closed_at: 2026-11-23T13:36:00Z
---

Goal: the release's notes and its GitHub release are made by `nx release changelog`, the way the maintainer's reference repositories zban and nx-rust do it, instead of GitHub's generated release notes.

Context: the maintainer decided on 2026-11-23, in his words: "I want the same nx generated release notes like on zban and nx release come on why you thing I sent them as examples?" This overturns the rejection of `nx release changelog` in ADR-DISPATCHED-RELEASE (docs/decisions.mara.md), which chose GitHub's generated notes. The release workflow (`.github/workflows/release.yml`, DES-RELEASE-WORKFLOW in docs/distribution.mara.md) keeps its jobs, permissions, recorded commit, environment `release` and resume guarantees: a re-run after a failure between the tag, the release and the asset uploads finishes the release, never fails on what already exists, never moves a tag and never publishes notes for another commit. The version stays set by hand in `Cargo.toml`; `nx release version` and `nx release publish` stay unused. `0.4.0-rc.1` was published with GitHub's generated notes; the orchestrator replaces its body with the notes that `nx release changelog 0.4.0-rc.1 --dry-run` gives.

- [x] `nx.json` gets a `release` block after zban's and nx-rust's pattern: the `mara` project, no commit scopes, tag pattern `v{version}`, no changelog file, a GitHub release, no commit
- [x] The Dry run job shows the notes from `nx release changelog "$VERSION" --dry-run`, read-only
- [x] The Tag job makes the tag and the GitHub release through `nx release changelog`, then attaches the assets, safe to re-run
- [x] DES-RELEASE-WORKFLOW and ADR-DISPATCHED-RELEASE record the decision; no sentence of ADR-NX-RUST-TASKS or AGENTS.md is made false
- [x] The notes of the published `v0.4.0-rc.1` are written to the orchestrator's state folder
- [x] A dry dispatch of the release workflow passes on the branch and shows the notes; `mara project validate` and `zban check` pass

## Summary of Changes

- `nx.json` has a `release` block after zban's and nx-rust's: project `mara`, `useCommitScope: false`, tag pattern `v{version}`, `automaticFromRef`, a workspace changelog with `createRelease: "github"` and no file, no project changelogs, git tag and push without commit. `nx release version` and `nx release publish` stay unused.
- `release.yml`: Dry run checks out every tag, runs the shared setup and `nx release changelog "$VERSION" --dry-run --verbose --git-push-args="--no-follow-tags refs/tags/$TAG"` with the read-only job token. Tag passes the same push arguments, so that Nx's leading `--follow-tags` sends no other tag. Tag looks up the tag on origin and the release before Nx: a release without its tag fails the job; with neither, Nx runs in full as `github-actions[bot]`; with the tag on the recorded commit and no release, it deletes the local tag copy and runs Nx with `--git-tag=false --git-push=false`; with both, it keeps the release and its notes; then it attaches the assets as before. Prepare's unused `prerelease` output is gone; Tag's timeout is 20 minutes for the setup.
- DES-RELEASE-WORKFLOW and ADR-DISPATCHED-RELEASE record the maintainer's choice, the range Nx takes (stable-tag precedence included), the author lookup that sends commit emails to ungh.cc and GitHub's user search with the setting that turns it off (left at Nx's default for the maintainer), the resume rules, the undocumented Nx 24.0.6 behaviours the Tag job relies on, and why version and publish stay unused; an open question on distribution.mara.md asks whether the 0.4.0 notes keep the previous implementation's 29 entries between `v0.3.0` and `d7f0e73`.
- The notes of `v0.4.0-rc.1` (range `v0.3.0..a118ba8`, 350 commits, 75 entries) are in `~/.local/state/zban-orchestrator/mara/release-notes-0.4.0-rc.1.md`, produced through Nx's programmatic API with the local tag deleted in a temporary clone.
- Checks: `pnpm run checks:fast` passed twice (second run 13/15 restored, the misses uncached); `mara project validate` 0 errors, 0 warnings; `zban check` no problems; actionlint and shellcheck clean but the existing `queue: max`. Dry dispatches https://github.com/convesoft/mara/actions/runs/45183528583 on 6d358eb and, after review round 1, https://github.com/convesoft/mara/actions/runs/45187464913 on 6cf0da0 (temporary version 0.4.0-rc.2 each time, reverted in 8ffa997 and the commit after 6cf0da0) passed; the second logs `git push --follow-tags --no-verify --atomic origin --no-follow-tags refs/tags/v0.4.0-rc.2`.

Rejected approaches: running `nx release changelog` unguarded on a re-run (git tag refuses the existing tag; with the tag present the range is empty); letting Nx update an existing release (would replace edited notes); the CLI's dry-run diff as the rc.1 notes (it elides lines common to the old body).

## Decisions made

- Release project `mara` only, the crate whose version the packages carry.
- Nx's defaults for the tag message and release title (`v<version>`) and for `strictPreid`: `0.4.0-rc.1` starts at `v0.3.0`, `0.4.0-rc.2` at `v0.4.0-rc.1`, `0.4.0` at `v0.3.0`.
- Tag runs `nx release changelog` only for what is missing, keeps an existing release and its notes, and fails on a release whose tag origin lacks.
- The Dry run passes the same `--git-push-args` as Tag, so its log shows the push Tag would make.
