---
title: Decide whether rule programs can carry code markers
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - maintainer
discovered_from: mara-cp1skx
created_at: 2026-11-21T21:19:51Z
---

Goal: the maintainer decides whether a requirement that a project implements in its Datalog programs can be linked to them, and how.

Context: found by mara-cp1skx (commit `b561961`), which linked this repository's code to its requirements. REQ-TEMPLATE-ACCEPTANCE-RULES and REQ-TEMPLATE-GATES-PROVE-READINESS are built wholly in `.mara/rules/*.dl`. DES-CODE-READERS (docs/code/index.mara.md) says a program is never a code file, so a marker in a `.dl` file is not read, and the gates `verified` and `release-ready` of the engineering template report these two requirements `not_implemented` for good. Any project that implements requirements in its programs meets the same. This adds or refuses a capability users see, so it is the maintainer's question; nothing waits for it.

Options, with the orchestrator's recommendation first:
- Accept it for 0.4.0: such requirements stay unlinked, and the docs of the template say so (recommended: no change to DES-CODE-READERS before the first release).
- Let a program file be declared as a code language too (a `[code.languages.datalog]` whose patterns match `.dl` files), which changes DES-CODE-READERS and what a program's hash covers.
- Link such requirements through an Accepted design item and a marker elsewhere; it does not help when nothing but the program implements them.

- [ ] The maintainer chooses; the choice is written into DES-CODE-READERS or the template's docs, and a follow-up issue is filed if it needs code
