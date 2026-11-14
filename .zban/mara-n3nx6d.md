---
title: Decide gate scope, unavailable outcomes and external records
status: todo
type: task
tags:
  - decision
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
---

Goal: settle the open questions on docs/rules.mara.md.

- [x] How outputs become invariants, gates or views: named configuration entries (DES-RULE-CONSUMERS, ADR-CONSUMERS-IN-CONFIGURATION)
- [x] How an invariant violation is reported (DES-DIAGNOSTICS, DES-RULE-CONSUMERS)
- [x] Records, ADTs and `autoinc()`: admitted; evaluation time is bounded instead (ADR-PROGRAM-PROFILE)
- [x] Whether `mara` offers a compile-only program check: no, `project validate` compiles every program (DES-RULE-EXECUTION)
- [ ] The scope and context of a gate, how a program reports an unavailable outcome, and how external records are supplied (open question on docs/rules.mara.md)
