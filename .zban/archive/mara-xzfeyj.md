---
title: "0.4.0-alpha.6: Node library and browser"
status: done
type: epic
blocked_by:
  - mara-779tms
  - mara-274e1m
  - mara-1fab1q
  - mara-c8scn7
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-25T18:30:35Z
---

Slice outcome: the same operations run from a Node library and in a browser, locally as WASM or through the daemon.

Done when one test suite passes against the embedded, daemon, HTTP and worker backends.

## Epic Closure (2026-11-25)

- All 13/13 child issues are closed on `main` (`4cbcf265c21fa120f8cab13e304cf15506a6c3ca`).
- `Done when` verified: the shared typed client parity suite (`crates/mara/tests/hosts/node/library/parity.test.mjs`, `mara::hosts library_parity`) runs against the embedded, daemon, HTTP, and browser-worker backends and passed on both Linux images in `main` CI run 45670123918.
- Release dry run on `main` passed: https://github.com/convesoft/mara/actions/runs/45670276066.
- Gemini architecture audit completed at `4cbcf26`: filed 3 bugs under `mara-qzp1gd` (mara-w7amcb, mara-8zm9hk, mara-332wqc), 9 backlog audit items (mara-dp5tn5, mara-mmr6mp, mara-rdx1w9, mara-5nnswe, mara-yqvt83, mara-rm6gye, mara-yyvqz7, mara-0np0gc, mara-2ctbmx), and updated `mara-fwy8zs`, `mara-dwytrs`, `mara-5d6pcs`. Also filed arm64 `library_timing` flake mara-zfn4jg under `mara-qzp1gd` and the 2 non-blocking questions from `mara-865a2e` (mara-h5fde2, mara-jv0311).
