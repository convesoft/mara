---
title: Compare CLI-direct and daemon results
status: todo
type: task
parent: mara-0mxw21
blocked_by:
  - mara-jkabqy
  - mara-xqcnqw
  - mara-v9tfhw
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:37:19Z
---

Goal: the first parity case of REQ-SAME-RESULT-ON-EVERY-HOST runs: each operation returns the same JSON in CLI direct mode and through the daemon.

Context: VER-HOST-COMPARISON in docs/verification.mara.md. Until this slice only its architecture checks run, which do not establish the requirement. The Node library and browser cases follow in alpha.6 (mara-vfkgbs).

- [ ] The daemon case of VER-HOST-COMPARISON passes for every operation the daemon serves
