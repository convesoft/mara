---
title: Release jobs install only what they use, and each platform builds mara in release mode once
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-23j2h9
discovered_from: mara-c8scn7
created_at: 2026-11-23T10:22:28Z
---

Goal: a release run does no setup and no build that its job does not use.

Context: architecture audit after rc.1. In `release.yml`, Registry check runs the whole shared setup (Rust with its components and target, cargo-nextest, pnpm and the frozen install) to call one Node script that with `--registry` never starts cargo; Pack dispatcher and Build install cargo-nextest and run no test; Checks compiles `mara` and its dependencies in the release profile for the performance test, uploads only Soufflé, and Build compiles them again on the same image and commit.

- [ ] Registry check needs Node only; the setup action can leave nextest out
- [ ] Checks hands the release binary it built to Build in the run's artifacts, or the item says why each job builds its own; `DES-RELEASE-WORKFLOW` keeps that every published binary is built in the same run from the recorded commit and packed after both Checks passed
