---
title: Release notes come from nx release changelog, as in zban and nx-rust
status: in-progress
type: task
parent: mara-xzfeyj
created_at: 2026-11-23T13:14:28Z
---

Goal: the release's notes and its GitHub release are made by `nx release changelog`, the way the maintainer's reference repositories zban and nx-rust do it, instead of GitHub's generated release notes.

Context: the maintainer decided on 2026-11-23, in his words: "I want the same nx generated release notes like on zban and nx release come on why you thing I sent them as examples?" This overturns the rejection of `nx release changelog` in ADR-DISPATCHED-RELEASE (docs/decisions.mara.md), which chose GitHub's generated notes. The release workflow (`.github/workflows/release.yml`, DES-RELEASE-WORKFLOW in docs/distribution.mara.md) keeps its jobs, permissions, recorded commit, environment `release` and resume guarantees: a re-run after a failure between the tag, the release and the asset uploads finishes the release, never fails on what already exists, never moves a tag and never publishes notes for another commit. The version stays set by hand in `Cargo.toml`; `nx release version` and `nx release publish` stay unused. `0.4.0-rc.1` was published with GitHub's generated notes; the orchestrator replaces its body with the notes that `nx release changelog 0.4.0-rc.1 --dry-run` gives.

- [ ] `nx.json` gets a `release` block after zban's and nx-rust's pattern: the `mara` project, no commit scopes, tag pattern `v{version}`, no changelog file, a GitHub release, no commit
- [ ] The Dry run job shows the notes from `nx release changelog "$VERSION" --dry-run`, read-only
- [ ] The Tag job makes the tag and the GitHub release through `nx release changelog`, then attaches the assets, safe to re-run
- [ ] DES-RELEASE-WORKFLOW and ADR-DISPATCHED-RELEASE record the decision; no sentence of ADR-NX-RUST-TASKS or AGENTS.md is made false
- [ ] The notes of the published `v0.4.0-rc.1` are written to the orchestrator's state folder
- [ ] A dry dispatch of the release workflow passes on the branch and shows the notes; CI passes; `mara project validate` and `zban check` pass
