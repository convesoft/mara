---
title: "`get` of a code path hidden by a problem on its way drops that problem's diagnostic"
status: done
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-h4z424
created_at: 2026-11-21T18:37:06Z
closed_at: 2026-11-21T19:06:27Z
---

Goal: `get <path>` of a path that a code include pattern matches, but that a problem on the way to it hides, reports that problem with its failure.

Context: found while fixing mara-h4z424. `get` in `crates/mara/src/cli/read.rs` asks `Inputs::code_file`, which returns the language and the diagnostics of the directories on the way to the path. When the language is `None` (the path is no code file), `get` reads the path as a document and drops those diagnostics. So with `src/.gitignore` unreadable or unsupported, or `src/x` a directory that cannot be listed, `get src/x/lib.rs` fails with not-found and does not say why. This was true before mara-h4z424 too, with the full listing's diagnostics. DES-CODE-READERS (docs/code/index.mara.md) says such a path "is read as a document after that check" and does not say which diagnostics the result carries.

- [x] Decide whether the not-found failure carries the diagnostics of the check on the way to the path, and write it in DES-CODE-READERS
- [x] A test: `get` of a code file under an unsupported `.gitignore` reports `unsupported-gitignore`

## Summary of Changes

Done in mara-h4z424, on the orchestrator's decision. `get` (`crates/mara/src/cli/read.rs`) attaches the diagnostics of the check on the way to its not-found failure, after the failure's own, when a path that a code include pattern matches is no code file. Such a path is never a document or an ID, so that `get` always fails. The rule is in DES-CODE-READERS and the case is a row of VER-CODE-MARKERS. The test is `get_of_code_hidden_by_a_problem_reports_it` (`crates/mara/tests/code/markers.rs`): `[z-a].rs` in `src/.gitignore` makes `get src/lib.rs` fail with `unknown-item` carrying `unsupported-gitignore` at line 1. The test fails without the fix. Checks: those of mara-h4z424.
