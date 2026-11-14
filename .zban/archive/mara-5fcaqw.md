---
title: Decide how VER-WORK-COUNTERS' warm rows run before the daemon exists
status: done
type: task
related:
  - mara-8d6cnw
  - mara-csmk0c
discovered_from: mara-km8qqj
created_at: 2026-11-14T13:26:44Z
closed_at: 2026-11-14T13:37:34Z
---

Goal: mara-8d6cnw has a test item it can pass in alpha.1.

Context: mara-8d6cnw asks that the VER-WORK-COUNTERS rows for an unchanged request, one edited document and unrelated changes pass in alpha.1 (docs/verification.mara.md). VER-WORK-COUNTERS runs the real CLI and daemon; the daemon arrives in alpha.4 (mara-0mxw21). ADR-MEMORY-ONLY-CACHES (docs/decisions-runtime.mara.md) keeps no on-disk store, so every CLI-direct request starts cold: it hashes every input and parses every document. Through the real CLI in alpha.1, an unchanged request cannot show 0 hashes and 0 parses, and REQ-WORK-FOLLOWS-CHANGE ("since the previous request") has no previous request in direct mode. Settling this changes how an Accepted requirement and its Verification apply, so a worker cannot decide it alone.

Options:

- A (recommended): in alpha.1, mara-8d6cnw asserts the cold counts through the real CLI (each declared input read and hashed once, each document parsed once, one snapshot build, `.git` and undeclared files never read) and the warm rows wait for the daemon slice. REQ-WORK-FOLLOWS-CHANGE states that it applies to hosts that keep state between requests.
- B: alpha.1 drives two requests through one in-process host session and asserts the warm rows there; the real-CLI rows wait for the daemon. Shows the incremental logic early, but not through the real workflow.
- C: CLI-direct mode keeps an on-disk cache. Contradicts ADR-MEMORY-ONLY-CACHES.

- [x] Option chosen and written into REQ-WORK-FOLLOWS-CHANGE or VER-WORK-COUNTERS
- [x] The VER-WORK-COUNTERS item of mara-8d6cnw names what alpha.1 asserts

## Summary of Changes

Decided by the maintainer (option A):

> yes in alpha.1, `mara-8d6cnw` asserts only the cold counts through the real CLI. The warm rows wait for the daemon slice (alpha.4), and REQ-WORK-FOLLOWS-CHANGE states that it applies to hosts that keep state between requests

- REQ-WORK-FOLLOWS-CHANGE (docs/performance.mara.md): a request derives only the inputs it reads, and undeclared files and `.git` cause no work. Limiting work to inputs that changed since the previous request applies to hosts that keep state between requests.
- VER-WORK-COUNTERS (docs/verification.mara.md): the rows are split into two groups. The cold rows run through the real CLI from alpha.1: a cold request, `.git` and undeclared files, and an operation that reads no code. The warm rows run against the daemon from alpha.4.
- mara-8d6cnw now asserts the cold rows. mara-p27z9t names the warm rows.

Checks: `zban check` reports nothing. The docs hand-check script and the coverage script report no problems.

## Decisions made

- The rows that hold in every host stay in REQ-WORK-FOLLOWS-CHANGE: a request derives only the inputs it reads, and undeclared files and `.git` cause no work. Only the part about change since the previous request is limited to stateful hosts.
- The cold request row counts the inputs the request reads, not every declared input, because an operation that reads no code reads no code file (DES-CODE-READERS). Its expectation is 1 file read and 1 hash per input, 1 parse per document and 1 snapshot build.
