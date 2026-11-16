---
title: "Fact bridge: mara.dl, JSON facts and request inputs"
status: in-progress
type: feature
parent: mara-17tdb3
created_at: 2026-11-13T22:10:06Z
---

Context: DES-FACT-BRIDGE in docs/rules-facts.mara.md; DES-REQUEST-INPUTS in docs/rules.mara.md; the generated wrapper in DES-RULE-EXECUTION (docs/rules-evaluation.mara.md); ADR-GENERIC-FACT-BRIDGE and ADR-GENERATED-JSON-IO.

- [ ] `mara.dl` format version 1, with only vocabulary-free relation declarations: items, titles, revisions, fields, folded text, edges
- [ ] One JSON file per fact relation, written once per document snapshot
- [ ] Inputs supplied by a request, inline or from a JSON file of rows; a missing input refuses the request
- [ ] Every supplied row checked before the run: column count, string for a symbol, signed 32-bit integer for a number; a wrong row refuses the request with its row and column (DES-REQUEST-INPUTS)
- [ ] `source_complete` from `--complete`
- [ ] Tests: the cases of VER-FACT-ROWS and VER-REQUEST-INPUTS (docs/verification-rules.mara.md) pass, natively; their browser rows wait for mara-xsm8w6
