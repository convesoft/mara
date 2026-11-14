---
title: Decide how mara-b52dg3 proceeds after two review rounds ended with the same defects
status: done
type: task
tags:
  - maintainer
parent: mara-t1rffa
related:
  - mara-b52dg3
created_at: 2026-11-14T14:30:04Z
closed_at: 2026-11-14T14:38:42Z
---

Goal: mara-b52dg3 "Load the project configuration and schema" can be integrated.

Context: the orchestrator parks an issue whose two review rounds ended with the same defect. The branch is `feat/load-project-config-schema` (worker thread "mara-b52dg3 Load the project configuration and schema"), head b8f8d27 before its rebase onto the integration tip.

What was tried:

- Round 1 (118c070), reviewer found: (1) a nested project can supply the selected project's schema when the outer configuration's `schema` points into it; (2) an invalid field kind hides independent schema errors (REQ-REPORT-EVERY-PROBLEM); (3) the DES-SCHEMA-MODEL example did not load cleanly.
- Fix (b8f8d27): `load` canonicalizes the schema path and refuses it (`outside-project`) when the real path leaves the project or lies under a nested project's root; `schema.rs` checks every part of a declaration before dropping it; the documented examples are tested from the page.
- Round 2 (b8f8d27), a new reviewer found the first two only partly fixed:
  - Isolation, through symbolic links: with `inner/.mara -> ../store` the canonical path no longer shows the nested root, so the outer project loads the inner schema; a schema symlink below a nested root pointing back into the outer project also passes; and `outer/.mara/config.toml` symlinked to `outer/inner/.mara/config.toml` makes the outer project read the nested configuration with no boundary diagnostic.
  - Hidden problems: two relations with the same `inverse`, the second lacking `targets`, report only `missing-key`; an invalid-kind field that shares a relation's name reports only the kind error.
  - Minor: the engine's packaged tests cannot compile because `documented.rs` includes `docs/project.mara.md` from outside the crate; a repeated forward/inverse name is reported at the earlier occurrence although DES-SCHEMA-MODEL promises the later one.
- Round 2 passed: the worker's decisions (refusing a schema reached through a symlink that leaves the project; a missing `format_version` is an error and the rest is read as version 1), the checks, the tracker and the documents.

Options:

- A (recommended): one more round by the same worker, with a simpler isolation rule set by the maintainer: Mara follows no symbolic link when it reads the project's own files. If `.mara`, `.mara/config.toml` or any component of the schema path below the project root is a symbolic link, loading refuses it with a diagnostic at the configuration line (line 1 for the configuration itself). This closes the whole class with one walk of the path as written and removes symlinked configuration and schema files as a possibility. For hidden problems: declared names are collected before declarations are validated, so collisions are reported whether or not a declaration survives; further combinations a later review finds become follow-up issues unless they lose a diagnostic for a problem the user must fix to get a valid project.
- B: one more round keeping symbolic links allowed where they stay inside the project: check the nested-project boundary at every component of the path as written and at every link hop, for the configuration and the schema. More code and more cases; the likeliest to need a fourth round.
- C: integrate with isolation for paths without symbolic links and file the link cases as a follow-up issue. Leaves a known gap in the isolation the maintainer decided; not recommended.

- [x] Option chosen and sent to the worker of mara-b52dg3

## Summary of Changes

Decided by the maintainer on 2026-11-14: option A, chosen as "A: no symlinks (Recommended)": "One more round by the same worker. Mara follows no symbolic link when it reads .mara, .mara/config.toml or any component of the schema path: such a path is refused with a diagnostic. Schema names are collected before declarations are validated; further hidden-error combinations found later become follow-up issues. Cost: no symlinked configuration or schema at all."

The orchestrator set mara-b52dg3 back to `in-progress` and sent the answer, with the open findings of the second review, to its worker in the same thread. The worker writes the rule into DES-PROJECT-CONFIGURATION. Check: `zban check`.
