---
title: At the grammar level, take a marker's comment leader from the grammar's comment node instead of comment_prefixes
status: todo
type: feature
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-znydv4
created_at: 2026-11-21T22:45:22Z
---

Goal: a language with a pinned grammar needs no `comment_prefixes` to find its markers: the grammar already says what a comment is, and the scan takes the comment's leader from the comment node.

Context: asked by the maintainer on 2026-11-21, after this repository pinned its Rust grammar (mara-znydv4, mara-egn2sb): "Do we need comment_prefixes then? Comments are resolved through grammar in this case isn't it?" Put in the backlog on the maintainer's word. Today `comment_prefixes` is still used at the grammar level for three things:
- `ScannedFile::scan_syntax` (`crates/mara-engine/src/code.rs`) takes the comment ranges from the grammar, then requires one of the prefixes at the start of each line of a comment node and reads the marker after it. A comment node holds its leader (`/// @mara implements X`), so without a matching prefix the line holds no marker.
- A file that its grammar cannot scan falls back to the basic level (`ScannedFile::fallback`), which finds comments by the prefixes alone.
- DES-CODE-MARKERS (docs/code/index.mara.md) requires at least one prefix per language, so a `[code.languages.<name>]` table without them is refused. `mara code add` writes the catalog's defaults when the language is not declared yet (DES-GRAMMAR-ASSETS).

This changes DES-CODE-MARKERS and what a configuration must hold, so it is a design change for the maintainer to accept before any code. Questions it has to answer: what the leader of a comment node is in every language (line comments, doc comments, each line of a block comment with or without `*`); whether `comment_prefixes` becomes optional when a grammar is pinned, and what the fallback to the basic level does then (the catalog's prefixes, or no markers with a diagnostic); and that the same file gives the same markers at both levels where both can read it (VER-GRAMMAR-MARKERS, VER-CODE-MARKERS in docs/code/verification.mara.md).

- [ ] DES-CODE-MARKERS says how a marker's start is found in a comment node without configured prefixes, and what `comment_prefixes` is for when a grammar is pinned
- [ ] The scan, the configuration check and `mara code add` follow it, with the verification rows for both levels
