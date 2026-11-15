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
- [ ] Write lock on the `.mara` directory; every target checked before the first write; temporary file, second check and rename; re-derivation of the changed files only in a host that keeps state, and none after the renames in a direct CLI call (DES-WRITE-PROCEDURE, ADR-NO-REDERIVATION-IN-CLI)
- [ ] The pause points for tests, in debug builds only: `MARA_TEST_PAUSE` names a directory; once per edit, after computing it and before step 2, the writer creates an empty `paused` there, and before each file's second check one holding the file's path; each time it waits until `paused` is gone. Plus the release-build `performance` case that finds no trace of them (DES-WRITE-PROCEDURE, ADR-DEBUG-PAUSE-POINT, decided by the maintainer)
- [ ] Tests below the CLI for the writer and the write procedure. Through the real CLI and its pause point, VER-PRESERVED-BYTES, VER-REFUSED-EDITS, VER-INTERRUPTED-EDITS and VER-CONCURRENT-WRITERS (docs/verification-writes.mara.md) and the writer row of VER-GITHUB-RENDERING (docs/verification-documents.mara.md) pass with each write operation, in the issue that ships it
