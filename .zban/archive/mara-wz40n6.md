---
title: Compare CLI-direct and daemon results
status: done
type: task
parent: mara-0mxw21
blocked_by:
  - mara-jkabqy
  - mara-xqcnqw
  - mara-v9tfhw
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:37:19Z
closed_at: 2026-11-21T11:01:50Z
---

Goal: the first parity case of REQ-SAME-RESULT-ON-EVERY-HOST runs: each operation returns the same JSON in CLI direct mode and through the daemon.

Context: VER-HOST-COMPARISON in docs/verification.mara.md. Until this slice only its architecture checks run, which do not establish the requirement. The Node library and browser cases follow in alpha.6 (mara-vfkgbs).

- [x] The daemon case of VER-HOST-COMPARISON passes for every operation the daemon serves

## Summary of Changes

- `crates/mara/tests/daemon/parity.rs` (new): the daemon case of VER-HOST-COMPARISON. The test starts a daemon with its own runtime directory (VER-DAEMON-START) outside a disposable engineering project, so the session opens with an earlier read that the daemon answers. It then starts a real `mara mcp` that sends its calls to that daemon. Each request runs in CLI direct mode, then from the CLI through the daemon, then as the MCP tool call through the daemon, and the test restores the files the request started from before each daemon run. Each daemon run must give the same exit status, stdout and stderr as direct mode, or the same text and `isError`, and leave the same files, once generated MIDs are paired. The workflow covers 22 operations: every document operation but `project init` and `project convert`, with writes, hand edits repaired by `project fix`, queries, gates and reports, failing and rejected requests, and a schema broken by hand.
- Each request was shown to have been answered by the test's daemon: `served` of `mara daemon status` rises by exactly one across each request from each client, and the daemon's PID is the same. Arguments the client rejects before it sends the request (`item list --limit 0`, `item move … --line 0`) leave the count unchanged. For those, `mara mcp` must answer as `mara mcp --no-daemon` does, because the MCP server names the parameter (`limit`) where the CLI names the option. A rejection made by the operation itself, such as `item update` with nothing to change, runs in the daemon and counts.
- An operation added later cannot be forgotten: the test fails when `tools/list` holds a document operation, other than `project_init` and `project_convert`, that it did not send. At the end, the daemon that opened the session must be the only daemon of the test.
- The daemon counts the operation requests it has run and answered, before each answer is sent. `mara daemon status` reports the count as `served` in JSON, null when no daemon runs, and in a sentence of its human form (`crates/mara/src/cli/daemon/server.rs`, `cli/daemon.rs`). This adds a field to that command's output.
- `crates/mara/tests/support/requests.rs` (new): what the MCP parity tests had in `tests/mcp/main.rs` and `parity.rs`, moved here so both test binaries share it. It holds `Files`, `files`, `Usage`, `usage`, `mcp_request`, `anchors` and `pair_mids`. `pair_mids` now accepts only a one-to-one pairing of MIDs and replaces them all at once. `tool_result` requires a `tools/call` response to have exactly one text block, a boolean `isError` and nothing else (DES-MCP-SERVER). Both MCP harnesses use it.
- The direct MCP harness (`tests/mcp/main.rs`, `writers.rs`) gives its processes the test process's temporary directory as `TMPDIR`, so their evaluations no longer write under `/tmp`.
- `crates/mara/tests/daemon/mcp.rs`: `Server::tools` added; `call_tool` is now `pub(crate)` and strict. `daemon/main.rs`: `mod parity;`.
- Docs: DES-DAEMON-LIFECYCLE gives `served`, VER-DAEMON-START uses it to tell that a call was served by the daemon, and VER-HOST-COMPARISON says how the daemon case checks coverage and service.
- No operation gave a different result through the daemon.

Checks run at the final head, all passed: `cargo fmt --all --check`, clippy with warnings denied, `cargo test --workspace --locked` (without `MARA_NO_DAEMON`), `project validate` (0 errors), the release performance test under `berth --exclusive`, the `wasm32` build of `mara-engine`, and `zban check`. With one request forced into direct mode by hand (`item update` from the CLI, with `MARA_NO_DAEMON=1`), the test failed on the count, and that change was not kept.

## Decisions made

- The test takes the operations to cover from the `tools/list` of `mara mcp`, which is one tool per document operation of DES-OPERATIONS (VER-CLI-MCP-PARITY). It leaves out `project_init` and `project_convert`, which run in the client's process (DES-DAEMON-LIFECYCLE). Written into VER-HOST-COMPARISON.
- Service is shown per request by a count that `mara daemon status` reports, which the orchestrator approved as the smallest addition. Nothing that already existed could show it: the PID in the daemon's reply does not reach the client's output, and the work counts of a cold request or a failure that does no work are the same in both modes. The count is global, not per session, and counts operation requests that the daemon ran and answered, not `status` or `stop` or the requests it refuses. Written into DES-DAEMON-LIFECYCLE, VER-DAEMON-START and VER-HOST-COMPARISON.
- The daemon starts outside the project, so the earlier request opens the session, as VER-HOST-COMPARISON says.
- An MCP call whose arguments the client rejects is compared with `mara mcp --no-daemon`, not with the CLI: DES-MCP-SERVER names the parameter in that message. Written into VER-HOST-COMPARISON.
- Between runs, the files are restored by rewriting only those that differ and removing those that were added, at the same path, instead of deleting and rewriting the whole tree. This is how a real workflow changes files under a watching daemon.
- The MCP test harness gets the test process's temporary directory as `TMPDIR`, not one inside the test's directory: the project is the test's directory there, and a temporary directory inside it would change the files that the parity tests compare.
