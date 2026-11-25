---
title: The tag tests' gh sends telemetry through the stub proxy
status: done
type: bug
parent: mara-xzfeyj
created_at: 2026-11-25T13:29:10Z
closed_at: 2026-11-25T13:35:00Z
---

`packages/mara-npm/tests/tag.test.mjs:191` failed once because `assertNoConnect` saw `CONNECT cafe.github.com:443`. gh 2.105 sends telemetry to `https://cafe.github.com` from a detached process (`telemetry/detach_unix.go`), so whether the CONNECT reaches the stub before the assertion depends on timing.

- [x] Set `GH_TELEMETRY=false` in the stub's environment (`packages/mara-npm/tests/support.mjs`), keeping `assertNoConnect` and the real gh
- [x] Local checks pass

## Summary of Changes

- `support.mjs` sets `GH_TELEMETRY=false` in the stub's gh environment (`gh help environment`: "Set to `false` or `0` to disable telemetry"). The gh binary has the endpoint `https://cafe.github.com` and `telemetry/detach_unix.go`. With `GH_TELEMETRY=log`, a stubbed `gh release view` prints a `command_invocation` telemetry payload. With `false`, it prints none.
- I could not make the CONNECT happen again on purpose: 3 runs against a loopback proxy logged none. The fix rests on the log and on gh's documented switch, not on a reproduced failure.
- Checks: `node --test packages/mara-npm/tests/*.test.mjs` passed 3 times (106 pass each). `pnpm run checks` under `berth --exclusive` with a fresh cache exited 0: 621 passed and 14 skipped in the workspace run, 32 passed and 2 skipped in the performance run, `validate` 0 errors and 0 warnings.

## Decisions made

- Switch off telemetry with `GH_TELEMETRY=false`, not `DO_NOT_TRACK`, because `GH_TELEMETRY` takes precedence.
