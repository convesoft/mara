---
title: "Guidance for rc.1: README installation with the next dist-tag and the migration steps from 0.3.0"
status: done
type: task
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
related:
  - mara-a4rbz4
created_at: 2026-11-22T18:44:35Z
closed_at: 2026-11-22T20:49:56Z
---

Goal: someone with a 0.3.0 project can install `0.4.0-rc.1` beside it and migrate the project by following the README and docs/migration.mara.md.

Context: the maintainer wants rc.1 to migrate other repositories from the 0.3.0 `:::mara` format. docs/migration.mara.md, docs/agents.mara.md, `DES-PACKAGES`. The first and third items of mara-a4rbz4 moved here for the prerelease; the rest of that issue stays for 0.4.0.

- [x] README: installation of the prerelease (`npx @convesoft/mara@next`, a project-local dependency), that `latest` is still 0.3.0 and reads another format, the platforms of rc.1, and the first workflow; within the README's size limit
- [x] docs/migration.mara.md: the steps from a 0.3.0 project to the new format with the commands of the worktree's binary as a user runs them, what the conversion changes and leaves alone, how to check the result (`project validate`), and how to go back
- [x] The steps are run once, end to end, on a copy of a real 0.3.0 project under `$TMPDIR` (the repository's own state at the tag `v0.3.0`, which the conversion tests already use); what failed or needed a manual step is in the page or filed
- [x] What an agent setup needs while both versions exist on a machine (the installed 0.3.0 skill and MCP server against a converted project), on docs/agents.mara.md, without changing the ban on the installed `mara` in AGENTS.md

Not part of this issue: user guidance on every topic page, `SECURITY.md`, the rules editor notes (mara-a4rbz4).

## Summary of Changes

- README.md: installation of `0.4.0-rc.1` from the dist-tag `next` (`npx`, or an exact project-local dev dependency), `latest` still 0.3.0 with the `:::mara` format, the rc.1 platforms (linked to DES-PACKAGES), first steps as a table; 41 lines. The stale alpha.1 status paragraph is gone.
- docs/migration.mara.md: `## Steps` at the top: convert (with `--status` when refused), review diff and report, the manual steps the report names (code links to markers, YAML rules to Soufflé, empty bodies), validate, agent setup, delete the 0.3.0 leftovers, and how to go back.
- docs/agents.mara.md: `## Both versions on one machine`: per-project pinned MCP servers, the 0.3.0 skill in project scope only, what each version does against the other's project.
- mara-a4rbz4: note on what moved here.

End-to-end run: `git archive v0.3.0` of the whole repository under `$TMPDIR`, `git init`, then the worktree's binary: `project convert` (no `--status` needed; 21 files, 32 report entries: 14 code links kept as text, project name, YAML rules, SCIP language, schema guidance of 11 flavours, 2 external-target relations), `project validate` 0 errors 0 warnings, rerun fails with `not-a-legacy-project`, the way back (restore docs, delete the three new files) gives a 0.3.0 project again, after converting again and deleting the leftovers validate stays clean, and validate through the daemon with a private `XDG_RUNTIME_DIR`. Nothing failed in the converter; nothing added to mara-gsdycs. Init → `item create` → validate → search also run on a new project.

Checks: `pnpm run checks` exit 0: validate 0 errors 0 warnings; 855 passed, 0 failed, 16 ignored.

## Decisions made

- The README installs project-locally with `--save-exact`, since a caret range on `0.4.0-rc.1` would accept later prereleases and 0.4.0 (README only; no item, it is npm behaviour).
- MCP servers for converted projects are pinned to the exact prerelease version, not `next`, as 0.3.0 pinned its version (docs/agents.mara.md).
- The migration tells users to delete `.mara/project.toml`, `.mara/schema.yaml`, the YAML rule files and `.mara/code/` after the conversion commit, in a commit of their own (step 8): the converter keeps them (DES-CONVERSION-MAPPING), no 0.4.0 operation reads them once `.mara/config.toml` exists (DES-PROJECT-CONFIGURATION), and while `project.toml` stays, a 0.3.0 tool reads the converted project as one without items instead of failing with "no project found" (verified in the v0.3.0 source, `src/lib.rs` discovery).
- The 0.3.0 skill is to be installed per 0.3.0 project, not per user (docs/agents.mara.md).
- The agent setup change (step 6) goes into the conversion commit (step 7), so one revert undoes both (docs/migration.mara.md).
- Legacy files under `.mara/` that Git does not hold are committed in step 1, ignored ones with `git add -f`, or copied out of the repository (docs/migration.mara.md).
