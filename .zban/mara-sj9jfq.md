---
title: project init with the engineering and empty templates
status: todo
type: feature
parent: mara-6k50w0
blocked_by:
  - mara-6ex559
created_at: 2026-11-13T22:10:06Z
---

Context: REQ-PROJECT-INIT in docs/project.mara.md; DES-ENGINEERING-VOCABULARY in docs/vocabulary.mara.md.

- [x] Decide which template applies when none is named: `engineering` (REQ-PROJECT-INIT)
- [x] Decide the inverse relation names (DES-ENGINEERING-VOCABULARY)
- [ ] Templates embedded in the binary; existing files are never overwritten; created files are reported

Schema source (from mara-37qtf2): the `engineering` template's schema must be the same text as this repository's `.mara/schema.toml`, which declares DES-ENGINEERING-VOCABULARY (docs/vocabulary.mara.md, descriptions included). Keep the one source inside the crate that embeds the template, for example `crates/mara/templates/engineering/schema.toml` read with `include_str!`, not in `.mara/`: a crate must not embed a file outside its package, and the product's template must not change with this repository's own configuration. The repository's `.mara/schema.toml` stays a regular file, since loading refuses a symbolic link in the schema path (DES-PROJECT-CONFIGURATION); a host test compares it, read-only, with what `project init` writes. The performance corpus schema, `crates/mara/tests/performance/schema.toml`, declares the same vocabulary with shorter descriptions and can then load the template's schema instead.
