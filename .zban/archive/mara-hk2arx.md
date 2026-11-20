---
title: Code links in programs and the template's code cases
status: done
type: feature
parent: mara-779tms
blocked_by:
  - mara-yhxq5x
  - mara-tx0fcf
created_at: 2026-11-16T17:22:19Z
closed_at: 2026-11-20T15:57:01Z
---

Goal: rule programs can use code links, and the engineering template's gates that need them say more than "unavailable".

Split by the orchestrator on 2026-11-16 from mara-yhxq5x, which builds the markers themselves. This part needs the program evaluation and query operations of alpha.3 and the template's programs (mara-tx0fcf).

Context: DES-CODE-READERS and DES-CODE-MARKERS in docs/code.mara.md; the code input relation in DES-FACT-BRIDGE (docs/rules-facts.mara.md); DES-TEMPLATE-GATES in docs/template-readiness.mara.md; VER-CODE-FACTS and VER-CODE-WORK-COUNTERS in docs/verification-code-use.mara.md; VER-TEMPLATE-CASES in docs/verification.mara.md.

- [x] Code links as an input relation for programs, and `source_complete("code")` (DES-CODE-READERS): VER-CODE-FACTS passes at the basic level
- [x] The code-dependent cases of VER-TEMPLATE-CASES pass: `implemented`, `verified`, `release-ready`
- [x] The rows of VER-CODE-WORK-COUNTERS for `query execute`, `query view`, `query gate` and `query report` of programs that use code links, and of programs that do not, pass
- [x] The grammar-level run of VER-CODE-FACTS, with the fixture grammars of VER-GRAMMAR-MARKERS (`Project::install_grammar` in `crates/mara/tests/code/main.rs`): mara-s9gak4 ran none of its cases, since each needs code links as program input. A row's location line and declaration name come from `CodeLinks::location`; a grammar that fell back still reads all code, so `source_complete("code")` holds

## Summary of Changes

- `mara.dl` declares `code_link`; `mara_engine::facts::CodeFacts` derives its rows from `CodeLinks`, and adds `source_complete("code")` when the code layer has no unread problem. `Program::names` records the relations that clauses name; `Planned::uses_code` says whether a run reads code links.
- `query execute`, `query view`, `query gate`, `query report` and `item validate` list code with the documents (`CodeReads::List`). They read and scan it (`Inputs::scan_listed`, which shares the per-file tracking with the walk) only for a program that uses code links, after admission and the input checks. `rules::consume`, `rules::consume_explained` and `rules::invariants` take the code facts through a `code` argument. A code file that cannot be read joins the request's diagnostics.
- `query report`: a `code_link` leaf is a fact node at its code file and the marker's line (DES-RULE-EXECUTION).
- Template: `readiness.dl` (both copies) reads `implements` rows of `code_link`.
- Performance corpus: each cluster's first source file implements the requirement that `query gate` judges, so the `verified` case passes at every size and seed. The report cases are `implementation-ready` (uses code links, reads all code) and `requirement-has-basis` (reads none) (DES-PERFORMANCE-HARNESS).
- Docs: DES-FACT-BRIDGE, DES-CODE-READERS, DES-TEMPLATE-QUERIES, DES-RULE-EXECUTION, DES-PERFORMANCE-HARNESS.
- Tests:
  - `code/facts.rs`: VER-CODE-FACTS at the basic level, and a `code_link` proof leaf.
  - `template/code.rs`: the code cases of VER-TEMPLATE-CASES, and the `source_complete("code")` input and absent leaves in reports. `Project::gate` also reports each failed or unavailable item of these cases.
  - `template/readiness.rs`: the fixture is now implemented by markers, so code no longer decides the outcomes.
  - `daemon/client.rs`: a query and a report over code links equal direct mode, before and after a marker moves.
  - `performance/cli.rs`: the query and report rows of VER-CODE-WORK-COUNTERS.
  - `program/tests.rs`: `names` in heads, bodies, negations, aggregates, groups, components and included files, not in comments or strings.

Checks: the seven local checks of AGENTS.md at the rebased head.

Not here: grammar-level runs of VER-CODE-FACTS (mara-s9gak4); an input named `code` (mara-e9va9c, backlog).

## Decisions made

- `code_link(rel: Rel, dst: Mid, path: Path, line: number, marker_line: number, declaration: symbol)` is declared in `mara.dl`, still format version 1 (not released). At the basic level `line` is the marker line and `declaration` is empty, so the grammar level changes values, not columns (DES-FACT-BRIDGE).
- A program uses code links when a clause in its own files (not `mara.dl`) names `code_link` unqualified, or an entry of its run consumes `code_link` (DES-CODE-READERS).
- `query` operations and `item validate` list the code files with the documents, and read and scan them only after admitting the program and checking the inputs, and only if the program uses code links. Other runs get an empty `code_link` (Soufflé rejects a declared relation with no input) and no `source_complete("code")`.
- `item validate` reads code when an invariant's program uses code links, so its findings match `project validate`'s. Added as a row of DES-CODE-READERS.
- `source_complete("code")`: added when every configured code file was read, including when `[code]` is absent. An invalid, unresolved or disallowed marker does not stop it. An unreadable file, or a directory or `.gitignore` that may hide one, does. A request that reads no code adds none (DES-CODE-READERS).
- The template's `implemented` reads only `implements` code links (DES-TEMPLATE-QUERIES). Both copies of `readiness.dl` are identical.

- `query report` reads code exactly as `query execute` does, and a `code_link` proof leaf is a fact at the code file and the marker's line (DES-RULE-EXECUTION).
- The performance corpus pins one `implements` marker for the requirement that `query gate` judges, so its `verified` case passes at every size (DES-PERFORMANCE-HARNESS).

## After mara-s9gak4 (grammar level)

Rebased onto `6064be4`.

- `Inputs::scan_listed` runs the listed code files through the walk's own `code_file`, `scan` and `rescan`, with the grammars loaded as `CodeReads::Scan` loads them. A grammar dropped during the request leaves every file of its language at the basic level, as `relation list` does. The function also returns the scan's diagnostics (`unavailable-grammar`, fallbacks, unreadable files), which `query` results include.
- `CodeFacts::new` takes a row's location line and declaration name from `CodeLinks::location`; `marker_line` stays the marker's line.
- Docs:
  - VER-CODE-FACTS: rows for a marker-like string, a grammar that falls back or is dropped mid-request, and the proof leaf at each level.
  - VER-CODE-WORK-COUNTERS: the grammar-level query row.
  - DES-CODE-READERS: a fallback leaves code complete.
  - DES-RULE-EXECUTION: a `code_link` leaf shows the marker's line at the grammar level too.
- Tests:
  - In `code/facts.rs`:
    - `at_the_grammar_level_a_row_holds_the_owning_declaration`: declaration line and name, no row for the raw string, rows equal to `relation list`, 1 code scan, code complete.
    - `a_grammar_that_falls_back_gives_basic_rows_and_code_stays_complete`.
    - `a_grammar_dropped_during_the_request_gives_basic_rows_for_every_file`, with the debug stall hook.
    - `at_the_grammar_level_a_code_link_leaf_is_at_the_marker`.
    - `a_process_loads_a_grammar_once_and_only_for_code_links`: in one `mara mcp`, a program without code links starts no grammar thread, and two code-link queries share one.
  - `performance::work_counters::grammar_query_counts`: with the grammar, `query execute` and `query report` of a code-link program and of a plain one count exactly as at the basic level.

### Decisions made in this round

- A grammar that falls back, for one file or its language, leaves code complete: every file was read and scanned, at the basic level (DES-CODE-READERS).
- A `code_link` proof leaf shows the marker's line at both levels; the row's values hold the declaration's line and name (DES-RULE-EXECUTION).

## Review fixes

The review of `5e56e07` found one defect: `item validate` ignored the diagnostics of the code it read. With a grammar pinned but missing from the cache and a warning invariant over `code_link`, it exited 0 as valid, while `project validate` reported `unavailable-grammar` and was invalid. Fixed in `cli/validate.rs`: when an invariant's program makes it read code, `item validate` adds to the item's problems:
- the scan's diagnostics (`unavailable-grammar`, per-file fallbacks, unreadable code files);
- the listing's problems that leave code incomplete.

These keep their severity, so an error makes the item invalid with exit status 1, and they are part of the result key. An `item validate` that reads no code reports none of them.

Tests:
- `code::facts::item_validate_reports_the_problems_of_the_code_it_read`: without a code-link invariant nothing about code is reported; with one, `unavailable-grammar` at the pin's line, invalid, exit 1, for a Need the invariant fails and for an item it does not; an unreadable code file gives its `io-error`. It fails with the fix removed.
- `code::facts::a_grammar_that_falls_back_gives_basic_rows_and_code_stays_complete` now also requires the `unavailable-grammar` of `query report`, as of `query execute`.

The other callers already report what they read: the query operations and `query report` add the scan's diagnostics to their results through one helper, `project validate` scans in its walk, and the daemon and MCP serve the same operations.

### Decisions made in the review round

- `item validate` that reads code reports the diagnostics of reading it as the item's problems, with their severity; one that reads no code reports none (DES-CODE-READERS).
