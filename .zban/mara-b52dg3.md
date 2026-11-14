---
title: Load the project configuration and schema
status: todo
type: feature
parent: mara-t1rffa
blocked_by:
  - mara-0ztabc
  - mara-b3wq5p
created_at: 2026-11-13T22:10:06Z
---

Goal: an operation finds its project and knows the project's vocabulary.

Context: docs/project.mara.md (REQ-ONE-PROJECT-PER-OPERATION, REQ-SCHEMA-DEFINES-VOCABULARY, DES-PROJECT-CONFIGURATION, DES-SCHEMA-MODEL). Verified end to end, through `project validate` (mara-000f9q), by VER-NESTED-PROJECTS and VER-CUSTOM-SCHEMA in docs/verification-documents.mara.md.

- [x] Decide where the document syntax version is recorded: `[documents] format_version` (DES-PROJECT-CONFIGURATION)
- [ ] Project selection by ancestor search and by `--project`
- [ ] Configuration loading; unknown settings are errors; a 0.3.0 project is refused and pointed to the conversion
- [ ] Schema loading: types, fields, relations with per-source target types
