---
title: Write the Verification items for the alpha.5 requirements
status: in-progress
type: task
priority: high
created_at: 2026-11-14T10:43:23Z
---

Goal: every requirement that the alpha.5 slice delivers has an accepted Verification item: the requirements that the issues of mara-779tms name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [ ] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [ ] One Verification item for each, or one shared by requirements that a single check covers
- [ ] The test items in those issues point at the Verification items instead of restating them

Started on 2026-11-16 before the alpha.4 epic closed, by the orchestrator's decision under the maintainer's rule of that day: this issue writes documents only, and code links do not build on the daemon. The alpha.5 epic stays blocked as it is.
