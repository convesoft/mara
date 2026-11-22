---
title: "CLI: mara --version prints the version"
status: todo
type: feature
parent: mara-c8scn7
discovered_from: mara-a57pzn
created_at: 2026-11-22T19:40:35Z
---

Goal: `mara --version` prints `mara <version>` and exits 0, as `DES-OPERATIONS` (docs/hosts/index.mara.md) now says, so that the install verification (`VER-INSTALL-WITHOUT-TOOLCHAIN`, docs/verification.mara.md), the registry check of `DES-RELEASE-WORKFLOW` and the epic's first "Done when" item can run it.

Context: found by mara-a57pzn. The epic's goal is `npx @convesoft/mara@0.4.0-rc.1 --version`, and 0.3.0's `mara --version` printed `mara <version>`, but the rewrite's CLI rejects `--version` with `invalid-argument` (exit 2). The version is the workspace version, `env!("CARGO_PKG_VERSION")` of the `mara` crate, which `mara mcp` already reports as `serverInfo.version`.

- [ ] `mara --version` prints `mara <version>` on stdout and exits 0. The top-level `mara --version` invocation takes no other argument (`DES-OPERATIONS`): another argument beside it, `--debug-stats` included, is rejected with `invalid-argument` (exit 2). An operation's own `--version` option, such as `code add <language> --version <version>` (`DES-GRAMMAR-ASSETS`), keeps its meaning
- [ ] The usage that bare `mara` prints names `--version`
- [ ] A Rust integration test runs the real binary for both cases and for `code add` with `--version`, which still reaches the operation; the version printed equals `serverInfo.version` of `mara mcp`
- [ ] Whether `DES-PERFORMANCE-HARNESS` needs a case without an operation for it, as for `mara --debug-stats`, decided and written there

Not part of this issue: the dispatcher and the packages (mara-svq540).
