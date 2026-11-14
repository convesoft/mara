---
title: "Fact bridge: mara.dl and TSV export"
status: todo
type: feature
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
---

Context: DES-FACT-BRIDGE in docs/rules-facts.mara.md; DES-REQUEST-INPUTS in docs/rules.mara.md; ADR-GENERIC-FACT-BRIDGE.

- [ ] `mara.dl` format version 1, with only vocabulary-free relations: items, titles, revisions, fields, folded text, edges
- [ ] One TSV file per relation, written once per document snapshot
- [ ] Inputs supplied by a request, inline or from a TSV file; a missing input refuses the request
- [ ] `source_complete` from `--complete`
