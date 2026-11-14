---
title: Decide gate scope, unavailable outcomes and external records
status: done
type: task
tags:
  - decision
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T10:43:23Z
---

Goal: settle the open questions on docs/rules.mara.md.

- [x] How outputs become invariants, gates or views: named configuration entries (DES-RULE-CONSUMERS, ADR-CONSUMERS-IN-CONFIGURATION)
- [x] How an invariant violation is reported (DES-DIAGNOSTICS, DES-RULE-CONSUMERS)
- [x] Records, ADTs and `autoinc()`: admitted; evaluation time is bounded instead (ADR-PROGRAM-PROFILE)
- [x] Whether `mara` offers a compile-only program check: no, `project validate` compiles every program (DES-RULE-EXECUTION)
- [x] The scope and context of a gate, how a program reports an unavailable outcome, and how external records are supplied (DES-GATE-OUTCOME, DES-REQUEST-INPUTS)

## Summary of Changes

The maintainer decided on 2026-11-14, with the recovered engineering-template design as input:

- A gate request names items; Mara derives each item's outcome from the gate's subject, condition and unavailable relations (DES-GATE-OUTCOME, ADR-MARA-COMPUTES-GATE-OUTCOME).
- External records are inputs that a program declares and a request supplies; `mara.dl` stays free of vocabulary (DES-REQUEST-INPUTS, DES-FACT-BRIDGE, ADR-GENERIC-FACT-BRIDGE).
- Queries, views, invariants and gates are named configuration entries, listed by `query list` (DES-RULE-CONSUMERS).

docs/rules.mara.md has no open question left. Checks: every item, MID and link in `docs/` checked by script (149 items, 242 links, no problems).
