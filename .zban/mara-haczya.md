---
title: Write the Verification items for the 0.4.0 requirements
status: todo
type: task
priority: high
blocked_by:
  - mara-xzfeyj
created_at: 2026-11-14T10:43:23Z
---

Goal: every requirement that the 0.4.0 slice delivers has an accepted Verification item: the requirements that the issues of mara-qzp1gd name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [ ] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [ ] One Verification item for each, or one shared by requirements that a single check covers
- [ ] The test items in those issues point at the Verification items instead of restating them

REQ-RUN-WITHOUT-TOOLCHAIN (docs/distribution.mara.md) belongs to this slice, not to alpha.3 (decided in mara-wamxz3): its Verification must include rule evaluation with the packaged Soufflé (DES-SOUFFLE-BUILD), run through `npx @convesoft/mara@<version>`.
