---
title: Parse only the code files that hold `@mara` at the grammar level
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-s9gak4
created_at: 2026-11-16T21:11:07Z
---

At the grammar level (mara-s9gak4) every file of a language with a grammar is parsed with tree-sitter's WebAssembly runtime, about 10 MB/s on the maintainer's machine, while the basic scan reads only files that hold `@mara`. A file without `@mara` has no marker, so its parse only serves `relation list <path>:<line>` and `get`, which name the declaration that owns a line.

- [ ] Skip the parse of a file without `@mara` in a scan of every code file, and parse it on demand when `relation list` names one of its lines, keeping VER-CODE-WORK-COUNTERS exact
- [ ] Measure a cold `project validate` on a large Rust tree with and without the skip before deciding
