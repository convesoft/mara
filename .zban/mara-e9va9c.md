---
title: An input named `code` lets a request mark code complete
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-hk2arx
created_at: 2026-11-20T15:12:19Z
---

Found while building code links in programs (mara-hk2arx).

`--complete <input>` adds `source_complete("<input>")` for any input the entry lists (DES-REQUEST-INPUTS). An entry whose program declares an input relation named `code` lets a request pass `--complete code`, which a program that uses code links cannot tell from Mara's own `source_complete("code")` (DES-CODE-READERS). With an unreadable code file, the template's `verified` gate could then fail a requirement as `not_implemented` instead of leaving it unavailable, or a project gate written the other way could pass.

Exotic: it needs a configuration that lists an input named `code`.

- [ ] Decide: refuse an input named `code` in the configuration (`invalid-relation`), or refuse `--complete code`
- [ ] Test: the request or entry is refused and names `code`
