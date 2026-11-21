---
title: Code links through the daemon
status: done
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
  - mara-xqcnqw
  - mara-gqvffn
  - mara-agdnwh
  - mara-h4z424
  - mara-7kfxn1
created_at: 2026-11-16T17:22:24Z
closed_at: 2026-11-21T19:36:13Z
---

Goal: a daemon serves code links warm: it watches the declared code files and re-scans only what changed.

Split by the orchestrator on 2026-11-16 from mara-yhxq5x, which builds the markers for direct mode and `mara mcp`. This part needs the daemon's sessions and watching (mara-xqcnqw, alpha.4).

Context: DES-CODE-READERS in docs/code.mara.md; DES-INPUTS-AND-KEYS in docs/engine.mara.md; VER-CODE-MARKERS (its daemon cases) in docs/verification-code.mara.md; the warm rows for code inputs in VER-CODE-WORK-COUNTERS (docs/verification-code-use.mara.md) and the "One code file edited" row of VER-WORK-COUNTERS (docs/verification.mara.md); VER-NEVER-STALE in docs/verification-daemon.mara.md.

- [x] The daemon watches declared code files; a request after a code file changed re-scans that file only
- [x] The daemon cases of VER-CODE-MARKERS and VER-CODE-EDITS (mara-yhxq5x ran them through the CLI and `mara mcp`), and the warm code rows of VER-CODE-WORK-COUNTERS and VER-WORK-COUNTERS pass
- [x] The daemon case of VER-HOST-COMPARISON covers the operations that read code

- [x] The daemon cases of VER-GRAMMAR-MARKERS and of VER-CODE-EDITS with grammars installed (mara-s9gak4 ran them through the CLI and `mara mcp`, in `crates/mara/tests/code/grammar.rs` and `edits.rs`); the daemon keeps a loaded grammar and reloads it only when its cached files' stats change (corrected after review: the code keeps loaded grammars in the process-wide registry of `crates/mara/src/grammar.rs`, `Registry`, which every request and project of the process shares, not in `Inputs`)

## Summary of Changes

Found already built: code files are session inputs (mara-xqcnqw); `tests/daemon/fresh.rs` `every_change_of_code_is_seen_by_the_next_request_over_code_links` checks that the daemon sees every change of code, but it did not check that the daemon served each request (fixed below). The daemon keeps loaded grammars in the process-wide registry and reloads one when its stats change (mara-gqvffn); `tests/daemon/grammar.rs` then tested only the sharing and retry of a load that fails. `get` rows warm (mara-b76x4g, mara-p27z9t, mara-agdnwh, `tests/performance/code_get.rs`). `tests/daemon/client.rs` compares code operations at the grammar level.

Added:
- Fix (`crates/mara/src/inputs.rs`, `refresh_ancestors`): a listing of the code files now keeps the records of every directory it lists in `State::ancestors`. Before, a walk that lists no code (`item list`, the watcher's derivation) dropped the code directories from the inventory, and the next request that read code read each of their `.gitignore` files again although they were unchanged (DES-INPUTS-AND-KEYS). Found by the new warm rows, whose corpus now has a `.gitignore` in a code directory.
- Daemon cases of VER-CODE-MARKERS, VER-CODE-EDITS and VER-GRAMMAR-MARKERS: in `tests/code/main.rs`, `Project::json` and `Project::problems` also send each request that a daemon serves to a daemon of the test. It runs on the files direct mode found and must print the same JSON and leave the same files, and `served` must rise by one. So every code test also runs through the daemon, the rename with grammars included. The grammar fallback tests now load the grammar in the daemon before they remove or damage it.
- Daemon case of VER-HOST-COMPARISON (`tests/daemon/parity.rs`): the project declares Rust and holds markers. Through the daemon from the CLI and from MCP: `project validate` with an unresolved marker, `relation list` of an item, a line and a file, `get` of a line and of a file, `get` of a missing code file, `query execute implemented`, a delete refused for a marker, and the rename that rewrites markers.
- Warm rows (`tests/performance/warm_code.rs`, new; the two warm code tests moved there from `warm.rs`). From VER-WORK-COUNTERS, one code file edited. From VER-CODE-WORK-COUNTERS:
  - a code file edited to the contents of another;
  - code edited, then a request that reads no code;
  - a document edited, then `relation list`;
  - comment prefixes changed, with Rust and Python declared;
  - a grammar added by `code add` from a loopback repository;
  - `item rename` that rewrites markers;
  - a code file added, removed or renamed;
  - `get` after a scan.

Checks: `cargo fmt --all --check`, clippy (warnings denied), `cargo test --workspace --locked`, `project validate` (0 errors, 1 warning in `docs/distribution.mara.md`, which was there before), release `performance` test, `wasm32` build of `mara-engine`, `zban check`. All exit 0.

## Changes after the review of 282f9ae

- VER-HOST-COMPARISON (`tests/daemon/parity.rs`): the project adds a warning invariant over `readiness.dl`'s `not_implemented`, which uses `code_link`. Sent from the CLI and from `mara mcp`, each answered by the daemon: `item validate`, `query view requirement-verification`, `query gate verified` and `query report verified`. After the re-check of d182dc5, each asserts a value that only a read of the code gives, on the result of direct mode, which the daemon's from the CLI and from MCP must equal. The view: REQ-COLD-READ-TIME counts 1 code link (0 without the read). `item validate REQ-HAND-WRITTEN`, a requirement that no marker names: the invariant's warning (none without the read, since `not_implemented` needs `source_complete("code")`). `query gate verified` and `query report verified` of REQ-HAND-WRITTEN: the condition rows `not_accepted` and `not_implemented` (`not_accepted` alone without the read: `code_unavailable` is hidden by the failure). Seen to fail, each at its own assertion, with the code read turned off for invariants, for gates alone and for reports alone in temporary changes to `src/rules/invariants.rs` and `src/rules/consumer.rs`.
- DES-GRAMMAR-ASSETS through a daemon (`tests/daemon/grammar.rs`, `the_daemon_loads_a_grammar_again_only_when_its_files_change`): three requests give one `load` event, the cached `.wasm`'s mtime changed gives one more, and the next gives none; each request is `served` by the daemon. Seen to fail against a temporary `grammar.rs` that reloads on every request (3 loads, not 1) and one that never reloads (1 load after the change, not 2).
- `tests/daemon/fresh.rs`: `same` requires that `served` rose by one across each daemon request, and that the same daemon answered. Seen to fail for `every_change_of_code_is_seen_by_the_next_request_over_code_links` with the client forced into direct mode. The 2 s sleeps are VER-NEVER-STALE's second run and the settling precondition, and claim no watcher delivery.
- `tests/code/main.rs`: the daemon comparison also requires the modes direct mode left, and that every file direct mode left as it was (bytes and mtime) is left so by the daemon; for a read, every file. Restoring between the modes restores modes too. Seen to fail with a mode and mtime changed after the daemon's run.
- Private environments: `mara mcp` children of the code tests get the test's `TMPDIR`; `Warm` (performance) and `Test` (daemon) give every child a private `HOME`, which a test can replace.
- `Written::distinct_code` builds only in debug builds, its only caller's.
- DES-DAEMON-SESSIONS: keeping a directory's record keeps no watch of it. VER-CODE-WORK-COUNTERS: the warm rename row asks no scan of written contents that a scan of the session already holds under the same key.

Checks after the changes, on the integration tip 61ed426: `cargo fmt --all --check`, clippy (warnings denied), `cargo test --workspace --locked`, `project validate` (0 errors, the earlier warning in `docs/distribution.mara.md`), the release `performance` test (32 passed, no warning), the `wasm32` build of `mara-engine` and `zban check`. All exit 0.

## Decisions made

- VER-HOST-COMPARISON: the daemon case's project declares an invariant over code links (`readiness.dl`'s `not_implemented`, a warning), so that `item validate` reads code there.
- VER-CODE-WORK-COUNTERS, warm rename row: limited to written contents new to the session; contents a scan already holds under the same key are not scanned again.

- DES-DAEMON-SESSIONS: a listing of the code files keeps the listings and `.gitignore` records of every directory it lists apart from the inventory, so that the next listing of the code files reads no unchanged `.gitignore` again.
- DES-DAEMON-SESSIONS: a directory that only code files can be in is watched from a request that lists the code files until the next derivation that lists none. That follows the existing rule "those the last inventory listed"; whether to watch such directories steadily or never is mara-enc837.
- VER-CODE-WORK-COUNTERS, warm row "`item rename` that rewrites markers": the write scans no code, since it reads back no code file (DES-CODE-READERS). The next request that reads code scans each distinct code content written, once.
- VER-HOST-COMPARISON: the daemon case's project declares a code language and holds markers.
