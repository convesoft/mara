---
title: Should release builds strip mara and mara.node to reduce native package size?
status: todo
type: task
tags:
  - maintainer
  - question
related:
  - mara-9a1zjp
discovered_from: mara-865a2e
created_at: 2026-11-25T18:30:35Z
---

Goal: decide whether release builds of `mara` and `mara-node` should strip symbols (`strip = true` or `strip = "symbols"` in `[profile.release]`) before 0.4.0.

Context: in `0.4.0-rc.2` (`mara-865a2e`), each Linux native package grew to ~23 MB packed / ~70–79 MB unpacked because unstripped `bin/mara.node` is ~32 MB alongside unstripped `bin/mara`. Stripping release builds would substantially reduce npm download and disk size, at the cost of symbol names in native backtraces unless debuginfo is kept separately.

Options:
- [ ] Strip symbols in `[profile.release]` before 0.4.0 (recommended; provisional: keep unstripped for rc.2).
- [ ] Keep release builds unstripped.
