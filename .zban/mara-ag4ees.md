---
title: "macOS arm64: Soufflé build, CI job and native package"
status: todo
type: task
tags:
  - deferred
parent: mara-qzp1gd
blocked_by:
  - mara-a4m588
  - mara-4dr2p2
created_at: 2026-11-14T09:57:08Z
---

Goal: 0.4.0 supports macOS arm64, which DES-PACKAGES in docs/distribution.mara.md lists, and nothing before this issue needs a Mac or a macOS CI runner.

Everything connected to macOS is deferred to this issue on purpose (maintainer's decision, 2026-11-14):

- GitHub is short of macOS runners. A job can wait about 30 minutes for one, which would slow down every pull request.
- The maintainer has no time to check anything on a Mac now.

Until this issue starts:

- No workflow has a macOS job. CI and the release workflow start with the rc.1 epic (mara-1avrq4, mara-a3aq0n), on Linux only.
- No other issue builds, tests or packages anything for macOS, or asks the maintainer to try something on a Mac.
- macOS arm64 stays a supported platform of 0.4.0. This issue delivers it; the release waits for it.

Tasks:

- [ ] Build the pinned Soufflé for macOS arm64, which was never tried (open question on docs/distribution.mara.md; build findings in EVD-SOUFFLE-TRIAL)
- [ ] macOS job in CI and in the release workflow
- [ ] The test suite passes on macOS, including the daemon (runtime directory, file watching) and input discovery
- [ ] Native package for macOS arm64 with the CLI, the Node addon and `souffle`
- [ ] Install smoke test for CLI and MCP on macOS arm64, without lifecycle scripts
- [ ] The settled bullet removed from docs/distribution.mara.md
