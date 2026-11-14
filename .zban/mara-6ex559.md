---
title: Item writer and write procedure
status: todo
type: feature
parent: mara-6k50w0
created_at: 2026-11-13T22:10:06Z
---

Goal: one writer for item syntax and one safe way to put bytes on disk.

Context: DES-ITEM-WRITER in docs/authoring.mara.md; docs/writes.mara.md (REQ-MINIMAL-REWRITE, REQ-REFUSED-EDIT-CHANGES-NOTHING, DES-WRITE-PROCEDURE).

- [ ] Writer for anchors, metadata tables and relation links. It never starts a file with `---`: a leading `---` always opens front matter, and an item in it is reported as `item-in-front-matter` and not read (DES-REGIONS in docs/format.mara.md, decided by the maintainer in mara-t6xn9z)
- [ ] Write lock on the `.mara` directory; every target checked before the first write; temporary file, second check and rename; re-derivation of changed files only (DES-WRITE-PROCEDURE)
- [ ] Tests: a refused or outdated edit leaves files byte-identical; untouched bytes survive an edit; two Mara processes editing one project at once lose no edit (REQ-WRITERS-SERIALIZED)
