---
title: The tag tests' gh sends telemetry through the stub proxy
status: in-progress
type: bug
parent: mara-xzfeyj
created_at: 2026-11-25T13:29:10Z
---

`packages/mara-npm/tests/tag.test.mjs:191` failed once because `assertNoConnect` saw `CONNECT cafe.github.com:443`. gh 2.105 sends telemetry to `https://cafe.github.com` from a detached process (`telemetry/detach_unix.go`), so whether the CONNECT reaches the stub before the assertion depends on timing.

- [ ] Set `GH_TELEMETRY=false` in the stub's environment (`packages/mara-npm/tests/support.mjs`), keeping `assertNoConnect` and the real gh
- [ ] Local checks pass
