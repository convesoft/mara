---
title: The CLI and the daemon read options the same way
status: todo
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
---

Goal: one invocation means the same in direct mode and when the daemon serves it.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: REQ-CLI-MCP-PARITY and DES-OPERATIONS (docs/hosts/index.mara.md), DES-DAEMON-LIFECYCLE and VER-DAEMON-START (docs/daemon/).

- `crates/mara/src/cli.rs:510` decides direct mode by scanning the raw arguments (`arguments.iter().any(|argument| argument == "--no-daemon")`), not from the parsed flags, so `--no-daemon` given as an option's value or after `--` is taken for the flag.
- `crates/mara/src/cli/client.rs:466` refuses a non-repeatable option that is given twice (`if !option.repeatable && args.value(option.name).is_some()`) when the daemon takes the request, while the CLI's own parser lets the last one win. The same command line then succeeds in direct mode and fails through the daemon.

Done when direct mode is decided from the parsed flags, the CLI and the daemon apply one rule to a repeated non-repeatable option (decide which, and record it in DES-OPERATIONS), and tests cover both paths.

- [ ] Direct mode comes from the parsed flag; `--no-daemon` as a value or after `--` is not the flag
- [ ] One rule for a repeated non-repeatable option in direct mode, through the daemon and through MCP, written into DES-OPERATIONS
- [ ] Tests for both, comparing direct mode with the daemon
