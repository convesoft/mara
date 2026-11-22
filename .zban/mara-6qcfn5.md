---
title: "Licence notices of the crates in the native packages: the in-repo generator or cargo-about?"
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-t71trn
discovered_from: mara-svq540
created_at: 2026-11-22T21:47:54Z
---

Goal: the maintainer confirms or changes how the native npm packages get the licence notices of the crates compiled into `mara`.

Context: mara-svq540 needed the notices for `DES-PACKAGES` (docs/distribution.mara.md). cargo-about, the usual tool, is not installed, and installing a tool on the machine or in the release workflow is the maintainer's decision. The worker wrote `packages/mara-npm/scripts/third-party-licenses.mjs`: it reads `cargo metadata --offline`, covers every normal and build dependency of `mara` (188 crates, 117 distinct texts; it can list more crates than are compiled in, never fewer), and fails packaging when a crate has no licence file unless a small table names the crate whose files cover it (14 internal Wasmtime crates map to `wasmtime`).

Options:

1. Keep the in-repo generator (recommended by the worker and the orchestrator; provisional until answered). No new tool; the table needs an entry when a dependency without a licence file appears, and packaging fails until it has one.
2. cargo-about, installed in the release workflow and on developer machines at a pinned version. The established tool with licence policy checks; one more tool to install and pin, and a configuration file.

Nothing waits on this: rc.1 can ship with option 1, and option 2 replaces one script.

- [ ] The maintainer has answered
