---
title: Decide how operations other than validation treat configuration and schema errors
status: done
type: task
tags:
  - decision
parent: mara-t1rffa
related:
  - mara-bmc9gc
  - mara-000f9q
discovered_from: mara-b52dg3
created_at: 2026-11-14T14:11:11Z
closed_at: 2026-11-14T14:44:43Z
---

Goal: every operation knows whether it runs on a project whose configuration or schema has an error.

Context: DES-PROJECT-CONFIGURATION (docs/project.mara.md) makes an unknown setting an error and has every other problem leave out only the setting or declaration it concerns, so `mara::project::load` (crates/mara/src/project.rs, mara-b52dg3) returns a partial configuration and schema with their diagnostics. Validation reports those diagnostics with the rest (REQ-REPORT-EVERY-PROBLEM; VER-SEEDED-PROBLEMS seeds an unknown setting and an invalid schema declaration). The docs do not say what `get`, `search`, `item list`, `schema get`, the write operations and `project fix` do with them. In 0.3.0 (`git show d7f0e73:src/host.rs`, `load_project_root`), every operation except validation refused such a project with the first error.

Options:

- A (recommended, as 0.3.0): every operation that reads documents or the schema, except validation, refuses a project whose configuration or schema has an error and reports those diagnostics. `prime`, `project init` and `project convert` do not depend on them.
- B: read operations run on what loaded and return the configuration and schema diagnostics with their result; writes and `project fix` refuse.

- [x] Choice written into DES-PROJECT-CONFIGURATION

## Summary of Changes

Decided by the maintainer on 2026-11-14: option A, chosen as "A: refuse the project": "Every operation that reads documents or the schema, except validation, refuses a project whose configuration or schema has an error and reports those diagnostics. prime, project init and project convert are unaffected."

- DES-PROJECT-CONFIGURATION (docs/project.mara.md) states the rule. No operation exists yet; `mara::project::load` returns the diagnostics an operation needs to refuse.
- mara-bmc9gc names the refusal in its test items, and mara-000f9q names that validation is not refused. Written on the branch of mara-b52dg3.

Checks: `zban check`.
