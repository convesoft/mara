---
title: Decide whether a project reads the documents of a project nested inside it
status: done
type: task
tags:
  - decision
related:
  - mara-b52dg3
  - mara-8d6cnw
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:26:50Z
closed_at: 2026-11-14T13:37:41Z
---

Goal: the input inventory knows what to do with a directory below the project root that holds its own `.mara/config.toml`.

Context: REQ-ONE-PROJECT-PER-OPERATION (docs/project.mara.md) says an operation never combines projects, but DES-PROJECT-CONFIGURATION and DES-INPUTS-AND-KEYS (docs/engine.mara.md) do not say whether the outer project's include patterns match documents inside a nested project. The previous implementation did not handle nested projects (`git show d7f0e73:ROADMAP.md` lists "nested boundaries" as future work). VER-NESTED-PROJECTS (docs/verification-documents.mara.md) leaves this case out until it is settled.

Options:

- A (recommended): the outer project's inventory skips every directory below its root that holds `.mara/config.toml` or `.mara/project.toml`, so no operation reads another project's documents.
- B: include patterns apply as written; the outer project validates the nested project's documents against its own schema, and identical MIDs or IDs in both become duplicates.

- [x] Choice written into DES-INPUTS-AND-KEYS or DES-PROJECT-CONFIGURATION
- [x] A case for it added to VER-NESTED-PROJECTS

## Summary of Changes

Decided by the maintainer (option A), in two messages:

> Yes I think we skip any nested projects

> If they inside theirs own .mara directory of course

- DES-PROJECT-CONFIGURATION (docs/project.mara.md): a directory below the project root whose `.mara/` holds `config.toml` or `project.toml` is the root of another project. Nothing below it is an input, even where a pattern matches. A `config.toml` outside a `.mara/` directory makes no project.
- VER-NESTED-PROJECTS (docs/verification-documents.mara.md): in the fixture, the outer project's include patterns match the inner project's documents. "Uses only a project" means that no result holds an item, narrative unit or diagnostic from another project's files, and that the file-read counter counts only that project's inputs. New cases cover both directions, a nested 0.3.0 project, and a stray `config.toml`.
- The inventory item of mara-8d6cnw points at the rule.

Checks: `zban check` reports nothing. The docs hand-check script and the coverage script report no problems.

## Decisions made

- The rule lives in DES-PROJECT-CONFIGURATION, not DES-INPUTS-AND-KEYS: that item defines the project root and the input declarations, it satisfies REQ-ONE-PROJECT-PER-OPERATION, and DES-INPUTS-AND-KEYS already links to it for what an input is.
- Isolation is measured in two ways: by what results contain, and by the file-read counter of DES-PERFORMANCE-HARNESS.
