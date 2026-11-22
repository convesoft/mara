---
title: "Guidance for rc.1: README installation with the next dist-tag and the migration steps from 0.3.0"
status: in-progress
type: task
parent: mara-c8scn7
blocked_by:
  - mara-a57pzn
related:
  - mara-a4rbz4
created_at: 2026-11-22T18:44:35Z
---

Goal: someone with a 0.3.0 project can install `0.4.0-rc.1` beside it and migrate the project by following the README and docs/migration.mara.md.

Context: the maintainer wants rc.1 to migrate other repositories from the 0.3.0 `:::mara` format. docs/migration.mara.md, docs/agents.mara.md, `DES-PACKAGES`. The first and third items of mara-a4rbz4 moved here for the prerelease; the rest of that issue stays for 0.4.0.

- [ ] README: installation of the prerelease (`npx @convesoft/mara@next`, a project-local dependency), that `latest` is still 0.3.0 and reads another format, the platforms of rc.1, and the first workflow; within the README's size limit
- [ ] docs/migration.mara.md: the steps from a 0.3.0 project to the new format with the commands of the worktree's binary as a user runs them, what the conversion changes and leaves alone, how to check the result (`project validate`), and how to go back
- [ ] The steps are run once, end to end, on a copy of a real 0.3.0 project under `$TMPDIR` (the repository's own state at the tag `v0.3.0`, which the conversion tests already use); what failed or needed a manual step is in the page or filed
- [ ] What an agent setup needs while both versions exist on a machine (the installed 0.3.0 skill and MCP server against a converted project), on docs/agents.mara.md, without changing the ban on the installed `mara` in AGENTS.md

Not part of this issue: user guidance on every topic page, `SECURITY.md`, the rules editor notes (mara-a4rbz4).
