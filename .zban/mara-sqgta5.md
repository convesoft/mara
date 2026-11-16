---
title: Check the evaluation's memory and output limits on macOS
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-bx82k0
created_at: 2026-11-16T17:12:33Z
---

DES-RULE-EXECUTION bounds a native evaluation with `RLIMIT_AS` (1 GiB) and `RLIMIT_FSIZE` (64 MiB) set on the `souffle` child in `mara::rules`. Both were tested on Linux only. macOS may not enforce `RLIMIT_AS`, and the Soufflé build for macOS arm64 was never tried (docs/distribution.mara.md, Open questions). The time limit holds on every host.

- [ ] Run the memory- and output-limit cases of VER-PROGRAM-FAULTS on macOS arm64; if `RLIMIT_AS` is not enforced, choose another mechanism and record it in DES-RULE-EXECUTION
