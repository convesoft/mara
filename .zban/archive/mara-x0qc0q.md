---
title: Soufflé's diagnostics name an included file only by its base name
status: cancelled
type: task
parent: mara-17tdb3
related:
  - mara-bx82k0
discovered_from: mara-wrbjte
created_at: 2026-11-16T12:45:33Z
closed_at: 2026-11-16T14:53:44Z
---

Goal: a compile error in an included program is reported at its project path, as DES-RULE-EXECUTION (docs/rules-evaluation.mara.md) requires, also when two included files share a base name.

Context: observed on 2026-11-16 with the static Soufflé 2.5 of the spike: for `p.dl` including `sub/a.dl`, which includes `b.dl` (resolved to `sub/b.dl`), the diagnostics say `in file a.dl` and `in file b.dl`, without the directory. Mapping a diagnostic back to `.mara/rules/sub/b.dl` is ambiguous when a program also includes another `b.dl`. The parser (mara-wrbjte) returns the list of files a program reads (`Program::files`).

- [ ] Decide how the host maps a diagnostic's file to a project path when base names repeat, record it in DES-RULE-EXECUTION, and test it with VER-PROGRAM-FAULTS's included-program case

## Reasons for Cancelling

Folded into mara-bx82k0.
