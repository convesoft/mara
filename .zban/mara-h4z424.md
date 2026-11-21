---
title: get of a code location checks only that file's directories and does not choose kept contents by time
status: todo
type: bug
parent: mara-779tms
blocked_by:
  - mara-0t6mpc
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: `get` of a code location in a session costs work for that file and its ancestor directories only, and which kept contents answer it does not depend on a clock.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. `crates/mara/src/inputs.rs` as mara-b76x4g, mara-p27z9t and mara-agdnwh left it. Related backlog: mara-cytrb4 (`code_file` clones the inventory's directory and `.gitignore` maps), mara-2ndg14.

- `Inputs::code_file` runs a full walk over every configured language's directories on every `get` of a code location, reads and hashes the `.gitignore` files under the code roots when the session has not listed code before, reports diagnostics of unrelated code subtrees, and discards what the walk learned.
- `Inputs::read_code` picks between the contents an earlier `get` kept and the contents a scan kept by comparing their check times (`scanned.checked() > got.checked()`). DES-DAEMON-SESSIONS (docs/daemon/index.mara.md) says a session never decides by time: a clock that stands still or steps back gives the wrong one.
- mara-agdnwh already forgets kept contents and scans of files that left; check that nothing of this issue is left there.

Items: DES-CODE-READERS (docs/code/index.mara.md), DES-DAEMON-SESSIONS, DES-INPUTS-AND-KEYS (docs/engine.mara.md), VER-CODE-WORK-COUNTERS (docs/code/verification.mara.md).

- [ ] `get` of a code location checks the path's ancestor directories and their `.gitignore` files, not every code directory, and keeps what it learned in the session; its diagnostics are those of that path
- [ ] One record of the latest checked contents per code path, or an order that is no clock, decides which kept contents answer
- [ ] Work-counter tests: a repeated `get` of a code location in a session that never listed code reads nothing again
