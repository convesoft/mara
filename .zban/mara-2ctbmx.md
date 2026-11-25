---
title: Split crates/mara/src/cli/daemon/server/http.rs (713 lines) into wire framing and authorization/dispatch
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-8zm9hk
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #13, wave: `split`).

`crates/mara/src/cli/daemon/server/http.rs` (713 lines) mixes ~250 lines of generic HTTP/1.x wire framing (`Head`, `Response`, `read_head`, `read_body`, `send`, `linger`, unit tests) with listener lifecycle, CORS/token/project authorization, and dispatch. Also switch line 98 (`/dev/urandom` via `File::open`) to `mara::system::random`.

- [ ] Extract HTTP/1.x framing and its unit tests to `crates/mara/src/cli/daemon/server/http/wire.rs`, keep listener/auth/dispatch in `http/mod.rs`, and use `mara::system::random`
