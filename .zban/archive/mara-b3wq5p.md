---
title: Decide the configuration and schema file syntax
status: done
type: task
tags:
  - decision
parent: mara-t1rffa
created_at: 2026-11-13T22:10:06Z
closed_at: 2026-11-14T09:10:46Z
---

Goal: settle the open questions on docs/project.mara.md so the loader can be built.

- [x] File syntax and location of the project configuration and the schema
- [x] `Status` values declared per type or once per schema
- [x] DES-PROJECT-CONFIGURATION and DES-SCHEMA-MODEL updated; the settled bullets removed

## Summary of Changes

The maintainer decided on 2026-11-14: `.mara/config.toml` and `.mara/schema.toml`, both TOML, each with its own format version starting at 1 (ADR-TOML-PROJECT-FILES); `Status` values are declared once per schema; field kinds are `text` and `enum`. DES-PROJECT-CONFIGURATION and DES-SCHEMA-MODEL in docs/project.mara.md hold the result, and the settled bullets are removed.

Checks: every item, MID and link in `docs/` checked by script (125 items, 193 links, no problems); every page within its size limit.
