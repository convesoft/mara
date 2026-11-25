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
- Tests: `crates/mara/tests/daemon/browser.rs` (7 tests, every case of VER-BROWSER-ACCESS except the Chromium one), plus unit tests of the origin form and the token comparison.

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

Checks: `pnpm run checks` (fresh `NX_CACHE_DIRECTORY`, `berth --exclusive`): 871 passed, 0 failed, 16 skipped (workspace 839 passed / 14 skipped, performance 32 passed / 2 skipped); `project validate` 0 errors, 0 warnings. `cargo clippy --workspace --all-targets` clean.

## Decisions made

- Wire format DES-BROWSER-PROTOCOL v1: `POST /operation`, body = native operation request without `build`/`cwd`/`umask`/`temp_dir` (unknown fields refused), `arguments`/`options`/`supplied` default to `[]`, `json` defaults to `true`. The answer is the native operation answer with `version` 1. Refusals are `{"version":1,"error","message"}`: 401 `refused` (token), 403 `refused` (origin, project), 404 `unknown-request`, 400 `malformed-request`, 503 `stopping`. Preflight returns 204 with `Allow-Methods: POST`, `Allow-Headers: Authorization, Content-Type`, `Max-Age: 600`.
- Browser requests run with the project root as working directory and with the umask and temp dir of the `start --http` process. `project` and `input` options are refused: a browser names no file of the daemon (`--input x=@file`), and inputs travel in `supplied`, as in MCP.
- Words that name no Mara operation are `malformed-request` (400). An existing operation that the daemon does not serve is `unavailable-operation` (200 answer, status 1, `served` not counted).
- `--allow-project` resolves from the working directory at start; a path that does not resolve or is no directory is `invalid-argument`. An origin must be in the form a browser serializes: lowercase host, no default port, no leading-zero port. IPv6 must be bracketed.
- ADR-HTTPARSE-LISTENER: `httparse` on `std::net`. Rejected: a hand-written parser, `tiny_http`, an async server.
- The listener closes within 100 ms of a stop and before exit.

## Question for the maintainer

- VER-BROWSER-ACCESS expects a write to A at `../<B>/docs/x.mara.md` to fail with `not-an-input`, "as in CLI direct mode". Direct mode refuses that path with `invalid-argument` (exit 2), because `..` leaves the root. The test requires the direct-mode result and an untouched B. I recommend changing the VER text to `invalid-argument`.
