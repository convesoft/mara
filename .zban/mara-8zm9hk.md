---
title: Browser HTTP listener validates via CLI client::args (leaking format and unvalidated supplied) and duplicates browser-unavailable lists
status: todo
type: bug
tags:
  - audit
parent: mara-qzp1gd
related:
  - mara-fwy8zs
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #2, wave: `seam`; aggravates `mara-fwy8zs`).

1. `crates/mara/src/cli/daemon/server/http.rs:448-459` rejects only `project` and `input`, then delegates to `cli/client.rs:459-502` (`client::args`), which chains `[PROJECT, FORMAT]` before `operation.options`. So a browser HTTP request with `options: [["format", "human"]]` is silently accepted even though `DES-BROWSER-PROTOCOL` exposes no `format` option.
2. `client::args` (`client.rs:489-500`) never checks whether `operation.options` declares `Kind::Inputs` — silently accepting and ignoring `supplied` on operations without inputs — and checks `!rows.is_null()` instead of `rows.is_array()` (diverging from `tool.rs:202` and `DES-BROWSER-PROTOCOL`).
3. `tool::call` (`cli/tool.rs:120-146`) and `Workspace::outcome` (`cli/workspace.rs:147-165`) duplicate `(name, &Value)` lookup, `Null`/`Object` validation, and single-project `project`-parameter rejection.
4. Browser-unavailable / in-process operation lists are duplicated in four places: `client.rs:42, 432`, `workspace.rs:30`, `packages/mara-browser/daemon.js:61`, and `parity.test.mjs:62`.

- [ ] Add capability flags directly on `Operation` in `crates/mara/src/cli.rs` (native + wasm32) so availability is derived from one table
- [ ] Separate CLI-global options (`PROJECT`, `FORMAT`) from operation-level validation in `client::args` (or validate HTTP requests through `cli/tool.rs`), reject non-empty `supplied` on operations without `Kind::Inputs`, require `rows.is_array()`, and extract a shared bound-project helper in `cli/tool.rs`
- [ ] Regression tests and local checks pass
