---
title: The CLI and the daemon read options the same way
status: done
type: bug
parent: mara-0mxw21
created_at: 2026-11-20T20:22:51Z
closed_at: 2026-11-21T09:53:29Z
---

Goal: one invocation means the same in direct mode and when the daemon serves it.

Context: Found by the maintainer's code audit of 2026-11-20 (at `a5f8ec0`, then `a5f8ec0..c6a9706`), filed at the maintainer's request. Locations were checked at `c6a9706`. Items: REQ-CLI-MCP-PARITY and DES-OPERATIONS (docs/hosts/index.mara.md), DES-DAEMON-LIFECYCLE and VER-DAEMON-START (docs/daemon/).

- `crates/mara/src/cli.rs:510` decides direct mode by scanning the raw arguments (`arguments.iter().any(|argument| argument == "--no-daemon")`), not from the parsed flags, so `--no-daemon` given as an option's value or after `--` is taken for the flag.
- `crates/mara/src/cli/client.rs:466` refuses a non-repeatable option that is given twice (`if !option.repeatable && args.value(option.name).is_some()`) when the daemon takes the request, while the CLI's own parser lets the last one win. The same command line then succeeds in direct mode and fails through the daemon.

Done when direct mode is decided from the parsed flags, the CLI and the daemon apply one rule to a repeated non-repeatable option (decide which, and record it in DES-OPERATIONS), and tests cover both paths.

- [x] Direct mode comes from the parsed flag; `--no-daemon` as a value or after `--` is not the flag
- [x] One rule for a repeated non-repeatable option in direct mode, through the daemon and through MCP, written into DES-OPERATIONS
- [x] Tests for both, comparing direct mode with the daemon

## Summary of Changes

- `crates/mara/src/cli.rs`: the parser records `--no-daemon` in `Args` where it reads the flag, and `run` decides direct mode from that instead of scanning the raw arguments. `--id --no-daemon` now names an ID and is served by the daemon. `-- --no-daemon` was already rejected as the unknown option `--`, and still is.
- New `Args::push` holds the one rule for an option's value: the option's own check, then a refusal of a second value of an option that does not repeat. The CLI parser (`cli.rs`), the daemon's re-check (`cli/client.rs`) and the MCP server (`cli/mcp.rs`) all use it. Each names the option its own way: `--limit` for the CLI, `limit` for the daemon and MCP.
- Finding: at `525168c` the CLI parser already refused a repeated option that does not repeat ("`--limit` is given twice"), so the audit's "last one wins" did not hold for the parser. Only `requested_json`, which picks the form of the error report, lets the last `--format` win. The divergence risk was two copies of the rule; there is now one.
- Docs: DES-OPERATIONS states that an option's value is the next argument whatever it reads as, that `--` ends no options, and the repeated-option rule for the CLI, the daemon and MCP. VER-DAEMON-START and VER-CLI-MCP-PARITY each gained a case.
- Tests: `cli::tests::reads_the_direct_mode_flag_only_where_it_is_a_flag` and extra rejection cases. In `tests/daemon/client.rs`, `arguments_mean_the_same_through_the_daemon_as_in_direct_mode` covers four things: a read with `--id --no-daemon` is served by the daemon with direct mode's result; the repeated `--limit` and `-- --no-daemon` are rejected as in direct mode; a raw operation request that repeats `limit` is refused by the daemon with the same reason; and the test fails with the old raw scan. In `tests/daemon/mcp.rs`, `a_repeated_option_is_rejected_in_direct_mode_and_with_a_daemon_alike` checks that `{"limit": [1, 2]}` gets the same `invalid-argument` text from a direct-mode server and from one that uses a daemon.

Checks, all exit 0: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked`, `project validate` (0 errors, 1 warning in docs/distribution.mara.md that was already there), the release performance test under `berth --exclusive`, the `wasm32` engine build, `zban check`.

## Decisions made

- Rule for a repeated option that does not repeat: the CLI (direct mode or with a daemon) and the MCP server reject it with `invalid-argument` (exit 2), and the last value never wins. The daemon checks a request again by the same rule and refuses one that repeats such an option with `malformed-request`, without running it; no client of its build sends one (review fix). This was already the CLI's and the daemon's behaviour and what the DES-OPERATIONS code table says ("repeated"), and it keeps a typo from silently overriding a value. Recorded in DES-OPERATIONS.
- In MCP such a parameter takes one value, so an array is a type error (`invalid-argument`). Recorded in DES-OPERATIONS.
- `--` is not an end-of-options marker; it is an unknown option. This was the existing behaviour, now written into DES-OPERATIONS.

Filed: mara-dxc5wm (`--debug-stats` and `requested_json` also scan the raw arguments: a different flag, and its rule for rejected arguments needs a decision) and mara-qdmsnd (duplicate JSON keys in an MCP `arguments` object: the last one wins inside `rmcp`/`serde_json`), both backlog.

Review fixes: DES-OPERATIONS and VER-CLI-MCP-PARITY now name `malformed-request` for a request sent to the daemon itself. The `--id --no-daemon` test decides "served by the daemon" by work counts, as VER-DAEMON-START defines it: the same request repeated 2 s later reports no work, which direct mode never does. It still fails with the old raw scan. Merged `chore/clear-worktree-for-rewrite` at `594e9a0` and reran all seven local checks: all pass.
