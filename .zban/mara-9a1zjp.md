---
title: Review the decisions workers made in alpha.6
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-xzfeyj
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.6 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.6 epic (mara-xzfeyj) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

- 2026-11-16: mara-274e1m (the Verification items for alpha.6) was started before the alpha.5 epic closed, since it writes documents only. Undo: not needed.

- 2026-11-25, provisional (question mara-2c6hfk): the browser package is built as `@convesoft/mara-browser`, private and named in its manifest only, and the Node library is imported from `@convesoft/mara`; nothing is packed or published under either before the maintainer answers. Rests on `DES-PACKAGES`, `REQ-NODE-LIBRARY`. Undo: rename the package in its manifest; move the client's files.
- 2026-11-25: mara-gn76br (`docs/` in the dispatcher) starts after mara-23j2h9, because both change the packer's file lists and the file check before publication; the maintainer asked only that it start after the version is `0.4.0-rc.2`. Undo: remove the edge.

- 2026-11-25, mara-fkkcad: the origin check of `daemon start --http` parses no host. It checks a fixed list (`DES-BROWSER-LISTENER`): the scheme, no path or wildcard characters, no upper case, a port that is not the default, a bracketed host through the standard library's IPv6 parser. The first version re-implemented a browser's URL parsing and was wrong both ways (review, P2). Undo: replace the list by a URL library's serialization.
- 2026-11-25, mara-fkkcad: `VER-BROWSER-ACCESS` expected `not-an-input` for a write that leaves the project through `..`; CLI direct mode answers `invalid-argument` (`DES-OPERATIONS`), so the Accepted item changed to that. The requirement holds: the request is refused and the other project is untouched. Undo: change direct mode and the item together.
- 2026-11-25, mara-fkkcad: the macOS `lsof` case of `VER-BROWSER-ACCESS` is not built; the item marks it as running from mara-ag4ees, which has a task for it (the maintainer's decision of 2026-11-14 defers everything for macOS).
- 2026-11-25, mara-jptfbq, after a second opinion (Gemini): the browser runs the `mara` library's own host code as WebAssembly over an in-memory filesystem, one per module instance (`ADR-MEMORY-FILESYSTEM`), not a second host around the engine crate. Rests on `REQ-SAME-RESULT-ON-EVERY-HOST` and the reason of `ADR-NODE-NATIVE-ADDON`. Undo: a filesystem handle passed through the host functions.
- 2026-11-25, provisional (question mara-aedd1d): the browser module has plain exports and no `wasm-bindgen`.
- 2026-11-25, mara-jptfbq: a read of a file's contents counts as a file read in the browser workspace as on every host; the worker's exemption for the memory filesystem was removed, so that the bound of `VER-WORK-COUNTERS` means something there. Undo: restore the sentence in `DES-BROWSER-WORKSPACE`.
- 2026-11-25, mara-jptfbq: with Cargo's target directory redirected, the browser package's build takes the module from that directory or fails; it never copies a module the build did not write. Same limit as mara-6z7bjs.
- 2026-11-25, provisional (question mara-w8zr8m): the release steps are scripts with tests (mara-23j2h9), where zban and nx-rust keep inline shell.
- 2026-11-25, mara-23j2h9: `ADR-DISPATCHED-RELEASE`'s line on npm's dry-run refusal was made precise on evidence (npm 12.2.1 refuses a dry run only of a published version without a prerelease part); the decision is unchanged.
- 2026-11-25, mara-xh6mp6: the daemon tests and `warm_scaling.rs` keep their second kill pass; only the code tests' `Daemon::stop` waits once, through a consuming `kill_once`.
