---
title: "Decide: sandbox the Soufflé process or accept the containment risk"
status: done
type: task
parent: mara-qzp1gd
created_at: 2026-11-14T11:52:52Z
closed_at: 2026-11-16T10:42:53Z
---

Goal: before the release, the maintainer settles the open question on docs/rules-facts.mara.md.

Context: RISK-SOUFFLE-REACHES-OUTSIDE (Unlikely, Severe: level HIGH in the template's matrix). The allow-list scan, the private directory and `-l ""` (DES-PROGRAM-PROFILE, DES-RULE-EXECUTION) mitigate it, but two independent reviews each found a way past the earlier scan (EVD-SOUFFLE-PROGRAM-REACH), and a memory fault in Soufflé is outside any scan. The maintainer chose on 2026-11-14 to keep an operating-system sandbox out of alpha.3 and decide here.

- [x] Either a Design for an operating-system sandbox around the `souffle` child process (Landlock on Linux, Seatbelt on macOS) with its issue, or an accepted Decision that `Accepts` the risk: the sandbox, on Linux, as mara-ptq6xc, which writes the Design; macOS goes with mara-ag4ees
- [x] Limits on output size and memory of an evaluation, or an explicit decision to do without them: a finite program wrote 8 MB in 15 ms in a review probe: limits, built by mara-bx82k0
- [x] The open question removed from docs/rules-facts.mara.md: by mara-bx82k0 (limits) and mara-ptq6xc (sandbox), each in its branch

## Summary of Changes

Decided by the orchestrator on 2026-11-16, under the maintainer's rule of that day that it decides on the maintainer's behalf. No code or docs changed here; the work is in the issues named.

- Limits: an evaluation gets limits on output size and memory besides time. Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT; it tightens, so it needs no confirmation. Built by mara-bx82k0. Undo: drop that item of mara-bx82k0.
- Sandbox: provisional. The `souffle` child process runs in an operating-system sandbox on Linux (mara-ptq6xc). The other answer, a Decision that accepts a risk of level HIGH, loosens a security requirement and is the maintainer's alone to choose; the sandbox is the answer that needs no such choice and can be undone. The maintainer may overturn it: cancel mara-ptq6xc and record a Decision that `Accepts` RISK-SOUFFLE-REACHES-OUTSIDE.

Both are listed for the maintainer in mara-46j65e. mara-ptq6xc now blocks the release workflow in place of this issue.
