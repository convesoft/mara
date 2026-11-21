---
title: mara code add
status: done
type: feature
parent: mara-779tms
blocked_by:
  - mara-s9gak4
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-21T12:15:27Z
---

Context: docs/code.mara.md (REQ-ONLY-CODE-ADD-DOWNLOADS, DES-GRAMMAR-ASSETS).

- [x] Built-in catalog and fetch from a given repository
- [x] Checksum verification, per-user cache, pin in the project configuration
- [x] VER-CODE-ADD passes, and its row of VER-CODE-WORK-COUNTERS: no other operation connects to another machine (the daemon's loopback listener is not such a connection)

- [x] Set ADR-GRAMMAR-TRUST-ON-FIRST-USE (docs/code.mara.md) to Accepted and replace "Chosen provisionally by the orchestrator … for the maintainer to confirm" by the maintainer's confirmation of 2026-11-16: asked to choose between trust on first use, a required checksum and catalog only, the maintainer chose trust on first use

## Summary of Changes

- `mara code add <language> [--repository <url> --version <version>]` (`crates/mara/src/cli/code.rs`, with `code/catalog.rs`, `code/fetch.rs`, `code/pin.rs`): a `Run::Command`, so the client runs it in-process; no daemon and no MCP tool serves it. It takes the write lock, reads the configuration, fetches `.wasm` and `tags.scm`, checks the checksum before writing anything, stores each file under a temporary name in its cache directory and renames it, then writes the pin through the write procedure only when it changes.
- The pin gains an optional `repository` (engine `GrammarPin`, `check_repository`, `is_grammar_version`, `is_language_name` in `crates/mara-engine/src/config.rs`). An invalid repository makes the pin `invalid-value`.
- Built-in catalog: `crates/mara/src/cli/code/catalog.toml`, embedded and empty. Debug builds read `MARA_TEST_GRAMMAR_CATALOG` instead.
- HTTPS: `ureq` 3 with only `rustls` (ring, WebPKI roots); ADR-UREQ-RUSTLS-FETCH. The release binary went from 28,305,392 to 31,084,104 bytes.
- Docs: DES-GRAMMAR-ASSETS (layout, catalog entry fields, behaviour with a pin, fetch limits, failure codes, result), an open question on the catalog's entries, ADR-GRAMMAR-TRUST-ON-FIRST-USE Accepted with the maintainer's choice of 2026-11-16, VER-CODE-ADD points at the layout, DES-PROJECT-CONFIGURATION names the repository.
- Tests: `crates/mara/tests/code/add.rs` (VER-CODE-ADD, loopback server of `tests/support/grammar_server.rs` counting connections; ignored test for real catalog repositories), `crates/mara/tests/performance/code_add.rs` (counter row 3 reads, 3 hashes, 0 parses, 0 scans, 0 snapshot builds; growth case; no other operation through the CLI or MCP connects). `code add` added to the lists in `tests/prime.rs` and `tests/mcp/protocol.rs`.

Checks: fmt, clippy (warnings denied), `cargo test --workspace --locked`, `project validate` (valid; the one warning, in docs/distribution.mara.md, is older), release performance test under `berth --exclusive`, wasm32 engine build, `zban check`: all exit 0. No coredumps.

## Decisions made

- Given repository layout: `<url>/<language>/<version>/grammar.wasm` and `tags.scm`, the cache layout without the checksum; `--version` is required with `--repository` and refused without it.
- A catalog entry holds two URLs (`.wasm` and `tags.scm`), since releases publish `tags.scm` only inside the source tarball.
- A language the configuration lacks is declared from its catalog entry (`include = ["**/*.<ext>"]`, its comment prefixes); with a repository only, it is refused with `unknown-grammar`.
- With a pin: no options fetch the pinned release (from the pin's repository, or the catalog when it has that version and checksum, else `unknown-grammar`); the same repository and version are checked against the pin; another repository or version is trusted on first use and replaces the pin. Cached files that match are not fetched again.
- Loopback means an IP literal in 127.0.0.0/8 or [::1]; `localhost` is refused. Credentials, query and fragment are refused.
- HTTPS follows at most 5 redirects, to https only, and honours the environment's proxy; loopback http follows no redirect or proxy. 64 MiB per file, 30 s to connect, 300 s per file. A `.wasm` without the WebAssembly magic is `fetch-failed`.
- New failure codes: `unknown-grammar`, `fetch-failed`, `checksum-mismatch`; a configuration with errors is `invalid-project`, one the pin cannot be written into in place is `unreadable-edit`.
- Result: `language`, `version`, `checksum`, `repository` (null for the catalog), `fetched`, `pinned`.

## Fix round after the review of 38c9d64

- One form of a repository (engine `repository()` → `Repository { url, secure, host, port }`): scheme and host lowercase, IPv6 shortest, default port dropped, trailing `/` removed. The CLI option and the pin are read into it once; `code add` compares, fetches and pins that form. Test: `holds_every_spelling_of_the_pinned_repository_to_its_pin`.
- Strict authority: host of letters, digits, `-`, `.` or an IP literal; port 1–65535 in 1–5 digits; no backslash. Before connecting, `ureq`'s own `Uri` parser must read the same scheme, host and port (`fetch::same_address`). Tests: `gives_each_repository_one_form`, `takes_https_repositories_and_plain_http_only_to_loopback_addresses`, `connects_only_where_the_check_and_the_client_agree`, `rejects_other_repositories_before_a_connection`.
- The configuration edit is planned before any connection (`unreadable-edit` first); a refusal after caching names the cache directory and no longer says nothing was written. Tests: `refuses_before_a_connection_a_pin_it_cannot_write_in_place`, `refuses_a_configuration_changed_at_the_pause_before_step_2`.
- The `.wasm` is validated by `wasmtime::Module::validate`, never compiled or run. An unusable proxy (unreadable or SOCKS) fails with `fetch-failed` before any connection. Test: `refuses_a_file_that_is_no_valid_module_and_an_unusable_proxy`.
- No-download test covers every usage operation but `code add` (from `mara`'s usage), the daemon's cycle with a request it serves, and every tool of `tools/list`, each with `HOME` and an empty cache, exit status asserted; the growth case times `code add` alone through `scaling::check_writes`.
- Pin values are written by the `toml` crate (`display` and `serde` features of `crates/mara`'s dependency; no new crate in `Cargo.lock`).
- VER-CODE-ADD's row narrowed to operations whose results carry diagnostics; mara-9r4749 deleted from this branch with `zban delete`.
- Debug-hook tests ignored in release builds. The tests keep the literal `$HOME/.cache` path: the binary exposes the cache root only through the process environment.
- Merged `chore/clear-worktree-for-rewrite` at 95b93b9. Release binary: 31,664,296 bytes.

Checks at the new head: fmt, clippy (warnings denied), `cargo test --workspace --locked` without `MARA_NO_DAEMON` (767 passed, 12 ignored), `project validate` (valid, the older warning in docs/distribution.mara.md), release performance test under `berth --exclusive` (25 passed), wasm32 engine build, `zban check`: all exit 0. No coredumps.

## Decisions made (fix round)

- Same repository: the one form above; the rest of the path counts as written, percent-encoding included.
- Orchestrator: every check of the configuration before a connection; a verified file may stay in the per-user cache after a later refusal, and the message says so.
- Orchestrator, provisional for the maintainer: an unusable proxy (unreadable, or SOCKS) fails `fetch-failed` naming the variable and scheme; the variables are `ALL_PROXY`, `HTTPS_PROXY`, `HTTP_PROXY` in either case, first set wins, `NO_PROXY` exempts; no SOCKS support. The check runs for loopback fetches too, although they use no proxy.
- Orchestrator: the `toml` crate writes pin values; its `display` feature exposes `Value` only together with `serde`, so both are enabled for `crates/mara`.
- Orchestrator: VER-CODE-ADD narrowed; `item delete`'s result gains no field.
- A redirect target must be `https://` and may carry a query; the limits on credentials, query and fragment apply to the URL the user or the catalog gives.
- The connect timeout runs after host resolution; the 300 s limit includes it.
- The extra rejected-alternative bullet in ADR-GRAMMAR-TRUST-ON-FIRST-USE is removed.

## Last round after the re-check of 4b467d9

- Any proxy variable that names a proxy the client cannot read, or a SOCKS proxy, fails every fetch with `fetch-failed` before any connection, whatever `NO_PROXY` says (`fetch.rs` `usable_proxy`); test case with `NO_PROXY` exempting the host added to `refuses_a_file_that_is_no_valid_module_and_an_unusable_proxy`.
- A host's final root dot is accepted and dropped in the one form (engine `repository`, and `fetch::same_address`); case in `gives_each_repository_one_form` and `connects_only_where_the_check_and_the_client_agree`.
- No-download test asserts each MCP server's exit status and that the daemon's `served` count rose by one across the probe it serves. The test server now counts behind a sentinel connection (FIFO listen queue), so a connection completed before `connections()` is always counted.
- VER-GRAMMAR-MARKERS names its fixtures as Rust 0.24.2 and Python 0.25.0 instead of the catalog's versions (written with the worktree's binary).
- Merged `chore/clear-worktree-for-rewrite` at 0fc00f2. Release binary: 31,669,056 bytes.

Checks at the new head: fmt, clippy (warnings denied), `cargo test --workspace --locked` without `MARA_NO_DAEMON` (770 passed, 12 ignored), `project validate` (valid, the older warning in docs/distribution.mara.md), release performance test under `berth --exclusive` (25 passed), wasm32 engine build, `zban check`: all exit 0. No coredumps.

## Decisions made (last round)

- Orchestrator, provisional for the maintainer like the proxy rule: an unusable proxy in any proxy variable fails the fetch whatever `NO_PROXY` says.
- A final root dot of a host names the same host and is dropped in the one form.
