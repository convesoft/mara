---
title: library_timing can exceed its 200 ms event-loop gap under parallel CI load
status: todo
type: bug
parent: mara-qzp1gd
discovered_from: mara-vfkgbs
created_at: 2026-11-25T18:30:35Z
---

`mara::hosts library_timing` failed once on `main` push CI run 45670123918 (`Checks (ubuntu-22.04-arm)`, job `139402812770`) with `AssertionError: the event loop waited 270.32206499999995 ms (noDaemon true)` (limit 200 ms; passed in 5 prior arm64 runs and on rerun).

Likely causes to investigate and fix:
- `crates/mara/tests/hosts/node/library/timing.test.mjs` starts a fresh Node child process (`library-child.mjs`), starts the 10 ms interval timer, and immediately calls `client.run("project_validate", ...)` as the very first call — so `packages/mara-npm/lib/index.js` runs synchronous `process.dlopen` on the large unstripped debug `libmara_node.so` inside the timed window.
- `library_timing` also runs inside `mara:test` concurrently with 629 other nextest tests on a 4-core runner.

- [ ] Measure which gap (first callback vs middle vs settle) caused the >200 ms delay on `ubuntu-22.04-arm`
- [ ] Fix the test or addon loading without loosening `VER-NODE-LIBRARY`'s 200 ms non-blocking guarantee
- [ ] Local checks pass
