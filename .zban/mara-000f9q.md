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
- [ ] The validation operations select and load the project with `mara::project::select` and `load` (crates/mara/src/project.rs, mara-b52dg3), which only host tests have exercised. Real-CLI tests cover, with the codes of DES-PROJECT-CONFIGURATION: selection from a subdirectory, from inside a nested project and with `--project` (absolute, relative, naming a non-root: `not-a-project`); `no-project` outside every project; `legacy-project` naming `mara project convert`; an unknown configuration setting (`unknown-key`) reported while the schema and documents are still checked; a schema file that does not exist (`io-error` at the configuration line naming it); an invalid schema declaration left out without hiding the other problems; a `format_version` other than 1 (`unsupported-format-version`); a schema path into a nested or 0.3.0 project (`outside-project` at the configuration line, the nested schema unread); a symbolic link at `.mara`, at `.mara/config.toml` or at a component of the schema path (`symbolic-link`, nothing behind it read); configuration and schema errors never refuse validation, which reports them with the rest (mara-zv9d69)
