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
- [ ] Test on the 0.3.0 corpus: `git show v0.3.0:docs/`
- [ ] Fixtures for the rest of DES-CONVERSION-MAPPING: relations written from the target, thematic breaks and setext headings in bodies, integer, number and boolean fields, empty bodies, an item without a status, and two values that become the same label (the conversion is refused)
- [ ] An include pattern of the 0.3.0 configuration that uses brace alternatives or a character class is kept as written and reported, since 0.4.0 patterns have neither and `{`, `}`, `[` and `]` match themselves (DES-CONVERSION-MAPPING, DES-PROJECT-CONFIGURATION; decided by the maintainer in the review of mara-8d6cnw). Fixture: `docs/{a,b}.mara.md` and `docs/[ab].mara.md`
