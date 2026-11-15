---
title: One-shot converter from :::mara
status: todo
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: docs/migration.mara.md (REQ-CONVERT-DIRECTIVE-CORPUS).

- [x] Decide the open questions on docs/migration.mara.md (DES-CONVERSION-MAPPING)
- [ ] Converter rewrites documents, schema and configuration as DES-CONVERSION-MAPPING describes, and reports what it leaves out
- [ ] VER-CONVERSION passes (docs/verification-hosts.mara.md), on the 0.3.0 corpus (`git show v0.3.0:docs/`) and the fixtures it lists, its interruption cases included: the report file `.mara/conversion-report.json` first, then the documents, the configuration last. A stopped conversion leaves a 0.3.0 project refused with `legacy-project` whose report file holds every omission of the documents already converted, and a rerun completes the conversion and returns the whole report. A rerun keeps every recorded entry and adds only entries the file does not hold, so a converted document edited by hand before the rerun keeps its omission and an unchanged unconverted one gets no duplicate (DES-CONVERSION-MAPPING, ADR-CONVERSION-RERUN, ADR-CONVERSION-REPORT-FILE, decided by the maintainer); for `project convert`, so do the cases of docs/verification-writes.mara.md, VER-INTERRUPTED-EDITS among them
- [ ] Work-counter test through the real CLI on a small hand-written 0.3.0 fixture, not generator output (the `project convert` row of VER-WORK-COUNTERS, DES-PERFORMANCE-HARNESS); no scaling test (ADR-NO-CONVERTER-SCALING, decided by the maintainer)
- [ ] An include pattern of the 0.3.0 configuration that uses brace alternatives or a character class is kept as written and reported, since 0.4.0 patterns have neither and `{`, `}`, `[` and `]` match themselves (DES-CONVERSION-MAPPING, DES-PROJECT-CONFIGURATION; decided by the maintainer in the review of mara-8d6cnw). A case of VER-CONVERSION
