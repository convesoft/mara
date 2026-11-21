---
title: One record per path for what a session holds of directories, ignore files and code
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-yv42pq
  - mara-2ndg14
  - mara-cytrb4
  - mara-enc837
  - mara-dskbzq
  - mara-dv7ad1
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:26Z
---

Goal: a session keeps one record per path of each kind (directory listing, `.gitignore`, document, program, code file), with one rule for when a record is reused, given back and forgotten, instead of four stores with a rule each.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); wave: seam. Locations are as of that commit. `State` in `crates/mara/src/inputs.rs` (near 78–96) holds what a session knows of paths in four places: `inventory`; `ancestors`, a second `Inventory` whose document and program maps are always empty; `code`; and `code_contents` with `code_scans`, written only by `get`. The reviews of alpha.5 found most of their defects here (mara-h4z424, mara-9fphtg), and the backlog issues mara-yv42pq, mara-2ndg14, mara-cytrb4 and mara-enc837 are consequences. None of the effects below gives a stale answer: every answer checks the stat. What the split costs today, by the auditor's reading (the orchestrator confirmed the first by reading):
- `read_code` (near 316–346) checks the file without the session lock and inserts into `code_contents` without checking that the file is still as read or that a newer check landed, unlike `give_back` (near 900): an overtaken `get` can replace a newer one's contents or put back a path that was forgotten, which costs a read later.
- `scan_listed` (near 549–569, 841–883) reads only `state.code`: after a `get` read, hashed and parsed a code file, the next `relation list`, `project validate` or `query execute` reads, hashes and parses it again.
- `scan_code` (near 377–382) finds an earlier scan by path only, where `Code.scanned` (near 1066) finds it by `MarkerKey`: a `get` of a renamed or copied file parses it again.
- A file edited in place after a `get` kept its contents leaves the old contents and scan in `code_contents` and `code_scans` until the file is deleted (memory).
- Every listing of the code files copies all directory and `.gitignore` records into `ancestors`, every other walk goes over `ancestors` to refresh it, and the walk reconciles the two maps (`held`, `Walk::kept`, `keep_ancestors`, `refresh_ancestors`).

The auditor's suggestion: one cache with a map per kind, valid while its record holds for the file's metadata, and a code record `{ contents, scan }` shared by `read_code`, `scan_code` and `scan_listed`, with the lookup by `MarkerKey` shared too; membership (which directories a session watches, which files a request's view holds) kept apart from the cache; one rule for merging an unlocked read back under the session's lock (only while the file is still as read); full walks evict what they did not visit. Items: DES-INPUTS-AND-KEYS (docs/engine.mara.md), DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), DES-CODE-READERS (docs/code/index.mara.md), REQ-NEVER-STALE, REQ-WORK-FOLLOWS-CHANGE, the decision of mara-agdnwh on how a session forgets code files (mara-7ztypp). High-risk: it is the session's freshness code.

- [ ] One record per path and kind; `ancestors`, `code_contents` and `code_scans` are gone, with `held`, `keep_ancestors`, `refresh_ancestors` and `Walk::kept`
- [ ] One rule gives an unlocked read back to the session, and one rule forgets; DES-DAEMON-SESSIONS states them
- [ ] The warm rows of VER-WORK-COUNTERS and VER-CODE-WORK-COUNTERS hold as before, and gain the rows for `get` then a listing of the code files (no second read, hash or parse) and for a `get` of a file whose contents the session already scanned under another path
- [ ] Say which of mara-yv42pq, mara-2ndg14, mara-cytrb4 and mara-enc837 this closes
