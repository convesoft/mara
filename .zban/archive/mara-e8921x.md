---
title: "CLI: mara --version prints the version"
status: done
type: feature
parent: mara-c8scn7
discovered_from: mara-a57pzn
created_at: 2026-11-22T19:40:35Z
closed_at: 2026-11-22T20:47:12Z
---

Goal: `mara --version` prints `mara <version>` and exits 0, as `DES-OPERATIONS` (docs/hosts/index.mara.md) now says, so that the install verification (`VER-INSTALL-WITHOUT-TOOLCHAIN`, docs/verification.mara.md), the registry check of `DES-RELEASE-WORKFLOW` and the epic's first "Done when" item can run it.

Context: found by mara-a57pzn. The epic's goal is `npx @convesoft/mara@0.4.0-rc.1 --version`, and 0.3.0's `mara --version` printed `mara <version>`, but the rewrite's CLI rejects `--version` with `invalid-argument` (exit 2). The version is the workspace version, `env!("CARGO_PKG_VERSION")` of the `mara` crate, which `mara mcp` already reports as `serverInfo.version`.

- [x] `mara --version` prints `mara <version>` on stdout and exits 0. The top-level `mara --version` invocation takes no other argument (`DES-OPERATIONS`): another argument beside it, `--debug-stats` included, is rejected with `invalid-argument` (exit 2). An operation's own `--version` option, such as `code add <language> --version <version>` (`DES-GRAMMAR-ASSETS`), keeps its meaning
- [x] The usage that bare `mara` prints names `--version`
- [x] A Rust integration test runs the real binary for both cases and for `code add` with `--version`, which still reaches the operation; the version printed equals `serverInfo.version` of `mara mcp`
- [x] Whether `DES-PERFORMANCE-HARNESS` needs a case without an operation for it, as for `mara --debug-stats`, decided and written there

Not part of this issue: the dispatcher and the packages (mara-svq540).

## Summary of Changes

- `mara --version` alone prints `mara <version>` (`CARGO_PKG_VERSION`, as `serverInfo.version` of `mara mcp`) on stdout and exits 0 (`crates/mara/src/cli.rs`, `run`).
- `--version` in option position before the parser has read a complete operation name (so also `mara project --version`), beside another argument, or with a value (`--version=1`), is rejected with `invalid-argument`, exit 2; `--debug-stats` still prints its stats line last on stderr. An operation's own `--version` (`code add … --version`) is parsed as before.
- The usage that bare `mara` prints has a second line, `mara --version`.
- New integration test `crates/mara/tests/version.rs`: the version equals `serverInfo.version` of a real `mara mcp` `initialize`; six rejected combinations; the usage line; `code add rust --repository <loopback> --version 0.24.2` fetches and pins that version from the test's server (HOME and cache under `$TMPDIR`, no internet). Unit cases added to `rejects_every_undeclared_or_malformed_argument` and `lists_every_operation_in_the_usage`.
- DES-PERFORMANCE-HARNESS: `mara --version` adds no case (decision below).

Checks: `pnpm run checks` (with `berth --exclusive`) exit 0: 859 passed, 0 failed, 16 ignored; `project validate` valid.

## Decisions made

- DES-PERFORMANCE-HARNESS: `mara --version` adds no case without an operation. It takes no other argument, so it never prints a stats line, and `mara --version --debug-stats` is rejected on the path of the existing `mara --debug-stats --no-such-option` case. Written into the item.
- Until the parser has read a complete operation name, `--version` in option position is the top-level flag, outside the document operation set; written into DES-OPERATIONS. With other arguments, or with an inline value, it is rejected with "`mara --version` takes no value and no other argument". After an operation's words it is that operation's option (or an unknown one).
- No `@mara implements` marker: `--version` is no document operation, so it carries out no distinct obligation of REQ-CLI-MCP-PARITY, the requirement DES-OPERATIONS satisfies; the rejection runs in `parse`, which already carries that marker.
