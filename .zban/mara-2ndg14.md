---
title: "`get` of a code location without markers reads its file again after a scan"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-p27z9t
created_at: 2026-11-21T12:56:56Z
---

Goal: the row "Another request, inputs unchanged" of VER-WORK-COUNTERS (docs/verification.mara.md) holds for the first `get <path>:<line>` of a code file without markers in a session that already scanned code: 0 file reads.

Context: mara-p27z9t made `Inputs::read_code` (crates/mara/src/inputs.rs) check the code file against the contents the session last kept of it: from an earlier `get`, or from a scan, which keeps contents only for a file with markers (`ScannedFile::new` in crates/mara-engine/src/code.rs). So after `project validate`, the first `get` of a code file without markers reads and hashes it once (1 file read, 1 hash); later ones read nothing. The 1× corpus's case names a file with markers and passes.

- [ ] Keep the contents of every scanned code file the session tracks, or record them from the scan in `State::code_contents`, within the session's memory
- [ ] A warm test: `project validate`, then `get` of an unmarked code file's line counts 0 file reads
- [ ] Update DES-DAEMON-SESSIONS (the bullet on code files)
