---
title: Item writer and write procedure
status: todo
type: feature
parent: mara-6k50w0
created_at: 2026-11-13T22:10:06Z
---

Goal: one writer for item syntax and one safe way to put bytes on disk.

Context: DES-ITEM-WRITER in docs/authoring.mara.md; docs/writes.mara.md (REQ-MINIMAL-REWRITE, REQ-REFUSED-EDIT-CHANGES-NOTHING, DES-WRITE-PROCEDURE).

- [ ] Writer for anchors, metadata tables and relation links
- [ ] Hash precondition, temporary file and rename, re-derivation of changed files only
- [ ] Tests: a refused or outdated edit leaves files byte-identical; untouched bytes survive an edit
