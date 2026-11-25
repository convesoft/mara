---
title: Review the decisions workers made in alpha.6
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-xzfeyj
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.6 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.6 epic (mara-xzfeyj) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

- 2026-11-16: mara-274e1m (the Verification items for alpha.6) was started before the alpha.5 epic closed, since it writes documents only. Undo: not needed.

- 2026-11-25, provisional (question mara-2c6hfk): the browser package is built as `@convesoft/mara-browser`, private and named in its manifest only, and the Node library is imported from `@convesoft/mara`; nothing is packed or published under either before the maintainer answers. Rests on `DES-PACKAGES`, `REQ-NODE-LIBRARY`. Undo: rename the package in its manifest; move the client's files.
- 2026-11-25: mara-gn76br (`docs/` in the dispatcher) starts after mara-23j2h9, because both change the packer's file lists and the file check before publication; the maintainer asked only that it start after the version is `0.4.0-rc.2`. Undo: remove the edge.
