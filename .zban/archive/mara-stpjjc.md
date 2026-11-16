---
title: Compile the admitted probes and the trial programs with the pinned Soufflé
status: cancelled
type: task
parent: mara-17tdb3
blocked_by:
  - mara-vt2ktq
related:
  - mara-bx82k0
discovered_from: mara-wrbjte
created_at: 2026-11-16T12:45:33Z
closed_at: 2026-11-16T14:53:44Z
---

Goal: the two rows of VER-ADMITTED-LANGUAGE (docs/verification-programs.mara.md) that need a Soufflé binary pass: every probe program the parser admits, and every rewritten trial program, compiles.

Context: mara-wrbjte added the parser and the fixtures in `crates/mara-engine/tests/programs/` without a Soufflé binary. Its test `tests/programs.rs` only checks, from the outcomes the review probes recorded natively, that no admitted probe printed a compile error. Each probe's `case.json` names its boundary relations (`boundary`) and the parser's verdict (`parser`); the trial programs are in `trial/` with Mara's declarations in `trial/mara.dl`.

- [ ] For each probe whose `parser` is `admitted`, and each program in `trial/`, a run of the pinned `souffle` (mara-vt2ktq) against empty facts, with a generated wrapper that reads and writes its boundary relations as JSON (DES-RULE-EXECUTION), exits 0 with nothing on stderr
- [ ] Runs in a temporary directory; skipped with a clear message, not passed, when the binary is not built

## Reasons for Cancelling

Folded into mara-bx82k0.
