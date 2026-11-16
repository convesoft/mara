---
title: Write the Verification items for the alpha.6 requirements
status: done
type: task
priority: high
blocked_by:
  - mara-779tms
created_at: 2026-11-14T10:43:23Z
closed_at: 2026-11-16T17:49:34Z
---

Goal: every requirement that the alpha.6 slice delivers has an accepted Verification item: the requirements that the issues of mara-xzfeyj name, and those satisfied by the designs they name. This issue blocks that epic, so it is the first work of the slice.

Context: AGENTS.md (Mara documents): a requirement is Accepted together with its Verification item, and during the rewrite a slice's Verification items are written before its work starts. The `engineering` template makes an accepted requirement without an accepted verification a validation error (DES-TEMPLATE-ACCEPTANCE in docs/template.mara.md); this repository keeps that invariant as a warning until the last slice. A Verification item defines how conformity is established (method and pass criteria), not test code. Existing ones are in docs/verification.mara.md.

- [x] List the requirements the slice delivers: those its issues name and those satisfied by the designs they name
- [x] Include REQ-SAME-RESULT-ON-EVERY-HOST, which no issue names
- [x] One Verification item for each, or one shared by requirements that a single check covers
- [x] The test items in those issues point at the Verification items instead of restating them

## Summary of Changes

Requirements the slice delivers and their Verification items:

| Requirement | Verification | Issue |
| --- | --- | --- |
| REQ-BROWSER-ACCESS-AUTHORIZED | VER-BROWSER-ACCESS (new, docs/verification-daemon.mara.md) | mara-fkkcad; Chromium case mara-vfkgbs |
| REQ-BROWSER-LOCAL-WORKSPACE | VER-BROWSER-WORKSPACE (new, docs/verification-browser.mara.md) | mara-jptfbq |
| REQ-UNAVAILABLE-NOT-PARTIAL | VER-UNAVAILABLE-NOT-PARTIAL (new) | mara-jptfbq, mara-xsm8w6, mara-vfkgbs |
| REQ-NODE-LIBRARY | VER-NODE-LIBRARY (new) | mara-vfkgbs |
| REQ-SAME-RESULT-ON-EVERY-HOST | VER-HOST-COMPARISON, extended with the Node and browser setup and cases | mara-vfkgbs; template-program row mara-xsm8w6 |
| REQ-FIRST-CALL-IN-PROCESS (Node client) | VER-DAEMON-START, Node rows added | mara-vfkgbs |
| REQ-INTERRUPTED-EDIT-LOSES-NOTHING (Node client) | VER-DAEMON-WRITES, Node row added | mara-vfkgbs |
| REQ-WRITERS-SERIALIZED (embedded Node) | VER-CONCURRENT-WRITERS, existing row | mara-vfkgbs |
| REQ-EXPLAIN-OUTCOMES, REQ-PROGRAM-FAULTS-REPORTED, REQ-PROGRAMS-ONLY-COMPUTE, REQ-EVALUATION-TIME-LIMIT (DES-RULE-EXECUTION) | Existing browser rows of VER-FACT-ROWS, VER-EXPLANATIONS, VER-PROGRAM-FAULTS, VER-PROGRAM-REACH-CASES, VER-EVALUATION-TIME-LIMIT | mara-xsm8w6 |
| REQ-RUN-WITHOUT-TOOLCHAIN (DES-PACKAGES, DES-SOUFFLE-BUILD) | Not here: mara-haczya (0.4.0), as decided in mara-wamxz3 | — |

New page docs/verification-browser.mara.md, listed in docs/index.mara.md. New design DES-BROWSER-WORKSPACE (docs/hosts.mara.md); rules added to DES-BROWSER-LISTENER, DES-DAEMON-LIFECYCLE, DES-OPERATIONS and DES-PERFORMANCE-HARNESS. The test items of mara-fkkcad, mara-jptfbq, mara-xsm8w6 and mara-vfkgbs point at these cases. Filed mara-p704wr.

Checks: `mara project validate` (worktree binary) valid; `zban check`; the local checks of AGENTS.md (see the commit's report).

## Decisions made

- Node tests run as `node --test` modules started by a Rust integration test, so `cargo test` runs them; the browser is Playwright's Chromium driven by Playwright's library, launched with a new profile; TypeScript's `tsc --strict --noEmit` checks the typings (VER-HOST-COMPARISON, VER-NODE-LIBRARY). Rejected: a hand-written DevTools driver (a runner of our own); a Node worker standing in for a browser (no browser).
- Browser workspace (DES-BROWSER-WORKSPACE): the caller may write and remove files; `project init` and `project convert` fail with the new code `unavailable-operation`, in the workspace and through the daemon; grammars are `unavailable-grammar` with basic-level scanning; an unloadable Soufflé module is a fault without a line; export leaves the workspace unchanged; an entry's `base` is the BLAKE3 hash in the form of `revision`, none for an added file; a file back at its supplied bytes has no entry.
- Browser listener (DES-BROWSER-LISTENER): `--http` needs both options and they need it; origins in serialized form only, `null` rejected; OS-assigned port; token of at least 128 random bits, sent as `Authorization: Bearer`, compared in constant time; exact `Origin` match; canonical project roots; token and origin checked before the path is used; a refusal opens no session and does not depend on the path; CORS headers only for allowed origins; only the daemon's document operations served; `daemon start --http` while a daemon runs exits 0 for the same options and otherwise fails with the new code `daemon-running`.
- A Node client's build is that of the `mara` binary of its native package (DES-DAEMON-LIFECYCLE); its debug stats option reports the stats object beside each result (DES-PERFORMANCE-HARNESS).

## Fixes after review

- VER-HOST-COMPARISON: each case needs only what its backend uses, so mara-jptfbq and mara-xsm8w6 pass their workspace cases without the addon or an HTTP daemon; the template-program row names its own workspace.
- VER-BROWSER-ACCESS: a refusal to an allowed origin carries the design's CORS headers; two origins and two projects allowed, an accepted read for each pair; the running-daemon rows add a changed project set (`daemon-running`, port, token and access policy unchanged).
- VER-NODE-LIBRARY: a daemon-backed case with the package's binary made non-executable, so no operation can start a CLI process; a recursive `any` check over every argument and result type in the same `tsc` run; the timer case bounds the largest gap (200 ms) instead of counting callbacks.
- VER-BROWSER-WORKSPACE: caller writes of the configuration, the schema, a `.gitignore` and a program, compared with CLI direct mode.
- VER-WORK-COUNTERS: four browser workspace warm rows (mara-jptfbq, mara-xsm8w6), read through the workspace's debug stats option, which DES-PERFORMANCE-HARNESS now names. Coverage adds REQ-WORK-FOLLOWS-CHANGE (browser workspace) → VER-WORK-COUNTERS.
- mara-p704wr made a backlog issue.
