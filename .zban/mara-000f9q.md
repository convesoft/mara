---
title: Validate projects and items
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-8d6cnw
created_at: 2026-11-13T22:10:06Z
---

Goal: `project validate`, `item validate` and `schema validate` report every built-in check failure with its location.

Context: docs/validation.mara.md (REQ-REPORT-EVERY-PROBLEM, REQ-VALIDITY-COVERS-WHOLE-PROJECT, REQ-READS-NEVER-WRITE, REQ-INVALID-ROWS-REPORTED, DES-SCHEMA-CHECKS, DES-DIAGNOSTICS).

- [x] Decide diagnostic codes and the result format (DES-DIAGNOSTICS)
- [ ] Built-in checks, linear in items plus edges
- [ ] CLI operations with human and JSON output; exit status reflects the whole project
- [ ] Scaling test covers validation
