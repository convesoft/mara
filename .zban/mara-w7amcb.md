---
title: JavaScript clients diverge on MaraError, debugStats, null options, and non-JSON/undefined inputs
status: todo
type: bug
tags:
  - audit
parent: mara-qzp1gd
created_at: 2026-11-25T18:30:35Z
---

From the post-`mara-xzfeyj` architecture audit at `4cbcf26` (Finding #1, wave: `seam`).

`MaraError`, `NO_WORK`, and `invalidArgument`/`refused` are duplicated across `packages/mara-npm/lib/index.js`, `packages/mara-browser/errors.js`, `index.js`, `worker.js`, and `daemon.js`, and the three JS clients disagree on four caller-visible contracts:
1. `Workspace.run` leaves `error.stats` `undefined` when `{ debugStats: true }` is passed on non-cloneable args (`index.js:78`), non-JSON args (`worker.js:173-179, 265`), or non-string `name` (`crates/mara-wasm/src/lib.rs:163, 354-364`), contrary to `DES-NODE-LIBRARY`.
2. `Workspace.run` (`index.js:92`) reads `options.debugStats === true` without optional chaining, throwing a raw `TypeError` when `options` is `null` (`Client` and `DaemonClient` use `options?.debugStats === true`).
3. `DaemonClient.run` (`daemon.js:218-222`) does not wrap `JSON.stringify` errors (e.g. `BigInt` or circular reference in `input`) in `MaraError("invalid-argument")`.
4. `DaemonClient.request` (`daemon.js:151-159`) rejects `{ a: undefined }` in `input` via `Object.entries`, whereas `Client` and `Workspace` serialize with `JSON.stringify` (omitting `undefined`).

- [ ] Consolidate `MaraError`, `NO_WORK`, and `invalidArgument(message, stats)` into one shared module
- [ ] Align all four behaviors across `Client`, `DaemonClient`, and `Workspace` with regression tests
- [ ] Local checks pass
