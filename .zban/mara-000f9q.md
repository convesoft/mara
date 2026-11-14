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
- [ ] CLI operations with human and JSON output
- [ ] VER-SCALING covers validation
- [ ] The cold rows of VER-WORK-COUNTERS (docs/verification.mara.md) pass through the real CLI for the validation operations: a cold request, `.git` and undeclared files, an operation that reads no code (moved here from mara-8d6cnw by the maintainer, mara-63dn2d)
- [ ] VER-SEEDED-PROBLEMS, VER-CUSTOM-SCHEMA and VER-GITHUB-RENDERING pass (docs/verification-documents.mara.md)
- [ ] The `project validate` case of VER-INVALID-ROWS passes, and VER-NESTED-PROJECTS and VER-UNCHANGED-SOURCES pass for the validation operations
