---
title: Daemon loopback HTTP for browsers
status: done
type: feature
parent: mara-xzfeyj
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-25T10:42:14Z
---

Context: REQ-BROWSER-ACCESS-AUTHORIZED in docs/daemon/index.mara.md.

- [x] Decide how the listener is enabled and how a browser obtains the session token (DES-BROWSER-LISTENER)
- [x] Loopback-only listener, started only by `mara daemon start --http`; port and token printed; such a daemon stays until `mara daemon stop`
- [x] Session token, allowed origins and allowed projects checked before any project access, as DES-BROWSER-LISTENER details them (origin form, Bearer token, CORS headers, served operations, `daemon start` while a daemon runs)
- [x] Tests: every case of VER-BROWSER-ACCESS (docs/daemon/verification.mara.md) passes but its Chromium case (mara-vfkgbs), and its inspection is recorded at review

## Summary of Changes

- `mara daemon start --http --allow-origin <origin> --allow-project <dir>` (both repeat): `start` binds `127.0.0.1:0` and draws a 256-bit token from `/dev/urandom` before it forks the daemon, which inherits the socket. `start` and `status` print `port` and `token` (JSON and human). Nothing is written to a file. A listening daemon never exits for idleness; its idle sessions are still dropped. `start --http` while a daemon of the build runs prints its port and token when origins and projects are the same sets, and otherwise fails with `daemon-running` (exit 1). A daemon started on demand never gets `--http`.
- The listener (`crates/mara/src/cli/daemon/server/http.rs`): HTTP/1.1, one request per connection, head parsed by `httparse` (already in the binary through `ureq`). Checks in this order: head, `Origin` (exactly one, byte-equal to an allowed origin), token (exactly one `Authorization: Bearer`, constant-time compare), `POST /operation`, body (JSON, known fields, `version`, `request`), project (absolute, canonical root in the allowed set), then operation and options. A browser request runs as a native operation request whose cwd and project are the canonical root, with the umask and temporary directory of the `start --http` process. `project init`/`convert` and every non-document operation answer `unavailable-operation`. The accept thread polls and closes the socket once the daemon stops, and the daemon joins it before exiting, so a connect after `daemon stop` returns is refused (a race that the test found).
- `--http` is the CLI's first operation flag (`Kind::Flag` in `cli.rs`).
- Docs: new DES-BROWSER-PROTOCOL (wire format version 1) and ADR-HTTPARSE-LISTENER in `docs/daemon/index.mara.md`. DES-BROWSER-LISTENER and DES-DAEMON-LIFECYCLE now name `port`/`token` and how `--allow-project` resolves.
- Tests: `crates/mara/tests/daemon/browser.rs` (9 tests, every case of VER-BROWSER-ACCESS but its Chromium case, and the macOS socket check, which is not run: it starts with mara-ag4ees), plus unit tests of the origin form, the token comparison and the 64 KiB head limit.
- Review round (Codex xhigh at fae736b, and GitHub's Codex review): an admitted browser request now stays counted until its answer is sent (`Admitted` in `http.rs`), so `daemon stop` sends a write's answer before exiting; test `a_write_whose_answer_is_unsent_at_stop_gets_it_before_the_daemon_exits`, which holds the answer at the debug-only pause point `MARA_TEST_PAUSE_ANSWER` (ADR-DEBUG-PAUSE-POINT) until the daemon removed its socket; with the release before the send it fails. The head limit is checked on a completed read too (unit test with a head of 64 KiB + 1 that one read completes). A body shorter than its `Content-Length` gets the 400 (`a_body_shorter_than_its_length_is_malformed`). The token refusals require status 401 and the exact body, so a path check before the token fails them. The idle/stop test scans for the token while the daemon runs and after, and requires each daemon gone after `stop`; the stale-token case checks `served` and sessions, the preflight `Max-Age` and `served`, `allows` no other `Access-Control-*` header, and the reads after a refused restart their CORS headers. Left: the 2 s settle of VER-DAEMON-START (no assertion here depends on cold or warm counts: each attribution is `served` + 1 under the same PID), the full refusal checks for the disallowed preflight and the stale token beyond `served`/sessions, and integration cases for the protocol's other malformed forms.

Statement of DES-BROWSER-LISTENER → test that fails when it is removed (each probed by a mutation, all caught):
- Token checked: `every_unauthorized_request_is_refused_alike`, `the_listener_outlives_the_idle_period_and_ends_with_stop` (previous run's token).
- Origin checked; no `Access-Control-*` for another origin: `every_unauthorized_request_is_refused_alike`, `a_start_while_the_listener_runs_changes_nothing`.
- Token and origin before the path, the project before anything in it is read: a refused request naming A must equal the same request naming a missing path (probe: token checked after the project → fails). If the project is not checked, B's word and a new session show up.
- A refusal has the same status and body whatever the path names and opens no session; `served` is unchanged and A, B, C stay byte-identical: `Browser::refused`, in every refusal case.
- Loopback only: `the_listener_serves_the_allowed_origins_on_the_allowed_projects` (`/proc/net/tcp*`; probe: bind `0.0.0.0` → fails).
- Nothing persisted: `the_listener_outlives_the_idle_period_and_ends_with_stop` searches every file under the test's directory (runtime dir, `HOME`, `TMPDIR`) for the token (probe: token written to the lock → fails).
- Daemon started on demand or by plain `start` never listens: `daemons_started_without_the_listener_never_listen` (probe → fails).
- Listener stays through the idle period: `the_listener_outlives_the_idle_period_and_ends_with_stop` (probe → fails).

Inspection for review (VER-BROWSER-ACCESS):
- Token from the OS generator, at least 128 bits: `bind` in `crates/mara/src/cli/daemon/server/http.rs` (32 bytes of `/dev/urandom`, hex).
- Constant-time comparison: `same` in the same file (length check, then an OR of the XOR of every byte, behind `black_box`). A plain `==` passes every test.
- Order of the checks: `respond` in the same file (origin, token, method and path, body, project, then `operation`). `read_head` copies the target but never uses it before the token is checked.

Checks after the review round, merged with `origin/main` at `0.4.0-rc.2`: `pnpm run checks` (fresh `NX_CACHE_DIRECTORY`, `berth --exclusive`): 875 passed, 0 failed, 16 skipped (workspace 843 passed / 14 skipped, performance 32 / 2); `project validate` 0 errors, 0 warnings; clippy clean. Before the review: 871 passed, 16 skipped.

## Decisions made

- Wire format DES-BROWSER-PROTOCOL v1: `POST /operation`, body = native operation request without `build`/`cwd`/`umask`/`temp_dir` (unknown fields refused), `arguments`/`options`/`supplied` default to `[]`, `json` defaults to `true`. The answer is the native operation answer with `version` 1. Refusals are `{"version":1,"error","message"}`: 401 `refused` (token), 403 `refused` (origin, project), 404 `unknown-request`, 400 `malformed-request`, 503 `stopping`. Preflight returns 204 with `Allow-Methods: POST`, `Allow-Headers: Authorization, Content-Type`, `Max-Age: 600`.
- Browser requests run with the project root as working directory and with the umask and temp dir of the `start --http` process. `project` and `input` options are refused: a browser names no file of the daemon (`--input x=@file`), and inputs travel in `supplied`, as in MCP.
- Words that name no Mara operation are `malformed-request` (400). An existing operation that the daemon does not serve is `unavailable-operation` (200 answer, status 1, `served` not counted).
- `--allow-project` resolves from the working directory at start; a path that does not resolve or is no directory is `invalid-argument`.
- The origin check (orchestrator's decision at review, written into DES-BROWSER-LISTENER): `http://` or `https://`, a non-empty host and an optional `:port`; no `/`, `?`, `#`, `@`, `*`, whitespace, control character or ASCII uppercase letter after the scheme; a port of 1 to 5 digits naming 1 to 65535, no leading zero, not the scheme's default; a bracketed host must parse as `Ipv6Addr`; nothing else of the host (a final dot passes). It replaces my first check, which re-implemented URL host rules.
- ADR-HTTPARSE-LISTENER: `httparse` on `std::net`. Rejected: a hand-written parser, `tiny_http`, an async server.
- The listener closes within 100 ms of a stop and before exit; a request accepted meanwhile gets 503 `stopping` at the operation, or no answer when the daemon exits first, and a write's answer is sent before it exits. DES-BROWSER-PROTOCOL now says so, that `text` is a JSON string, that requests are HTTP/1.0 or 1.1 and responses HTTP/1.1, and the time bounds (10 s per read and write, no whole-request limit, a thread per connection).
- VER-BROWSER-ACCESS changed (orchestrator's decision at review): the write to A at `../<B>/docs/x.mara.md` fails with `invalid-argument`, not `not-an-input`, as in CLI direct mode, where `project_path` refuses a `..` part with `invalid-argument`, exit status 2 (`crates/mara/src/cli/author.rs`; DES-OPERATIONS defines the code for a malformed argument). The test requires that code and status by name. The requirement is unchanged: the request is refused and B is not written.
- VER-BROWSER-ACCESS runs its macOS `lsof` check from mara-ag4ees, which has the task; nothing for macOS is built here. The HTTP listener's connections are recorded under mara-n1y6we.

## Questions for the maintainer

- None open: the sibling-path expectation was settled at review (above).
