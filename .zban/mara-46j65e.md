---
title: Review the decisions workers made in alpha.3
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-17tdb3
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.3 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.3 epic (mara-17tdb3) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf under the rule of 2026-11-16: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-16, provisional, from mara-t2qrcb: the `souffle` child process runs in an operating-system sandbox on Linux (mara-ptq6xc), instead of a Decision that accepts RISK-SOUFFLE-REACHES-OUTSIDE. Rests on that risk (level HIGH), REQ-PROGRAMS-ONLY-COMPUTE and EVD-SOUFFLE-PROGRAM-REACH. Undo: cancel mara-ptq6xc and record a Decision that `Accepts` the risk.
- 2026-11-16, from mara-t2qrcb: an evaluation gets limits on output size and memory besides time (an item of mara-bx82k0). Rests on REQ-PROGRAMS-ONLY-COMPUTE, RISK-SOUFFLE-REACHES-OUTSIDE and REQ-EVALUATION-TIME-LIMIT. Undo: drop that item of mara-bx82k0.
- 2026-11-16, provisional, from mara-b60px3: the four views the first `engineering` template leaves out (outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact) are deferred to after 0.4.0 (backlog mara-7h4z9f); mara-tx0fcf says so in DES-TEMPLATE-VIEWS. Rests on DES-TEMPLATE-VIEWS, which ships three views, and on a view being addable later without breaking a project. Undo: pull mara-7h4z9f, or one view of it, into a slice.

### Confirmed by the maintainer on 2026-11-16

The two provisional decisions above and one more were put to the maintainer as questions; each answer is the chosen option with its description, word for word. They are no longer provisional.

- Sandbox (mara-ptq6xc). Question: "Provisional decision 1 of 3, nothing waits on your answer. A rule program from an untrusted checkout could get past Mara's scans or hit a memory fault in Soufflé and then read or write files as the user (risk level HIGH in the docs). I decided to run the `souffle` child process in an operating-system sandbox on Linux, as its own issue before the release. Confirm?" Answer: "Sandbox on Linux (Recommended) — Keep my decision: a Landlock sandbox around the child process on Linux, built before 0.4.0; macOS follows with the macOS work. Costs one issue of release work, and closes the risk instead of accepting it." Not chosen: accepting the risk; a sandbox after 0.4.0.
- Four views (mara-7h4z9f). Question: "Provisional decision 2 of 3, nothing waits on your answer. The first `engineering` template ships three views (delivery readiness, requirement verification, risk treatment). You asked to come back to the other four designed views before the release: outcome coverage and validation, engineering basis and rationale, knowledge validity, change impact. I deferred all four to after 0.4.0. Confirm?" Answer: "Defer all four (Recommended) — Keep my decision. A view can be added in a later release without breaking any project, and 0.4.0 ships sooner. They are collected in one backlog issue." Not chosen: shipping some in 0.4.0; dropping them.
- Explanations (mara-wamxz3, which amends REQ-EXPLAIN-OUTCOMES). Question: "Provisional decision 3 of 3, nothing waits on your answer. REQ-EXPLAIN-OUTCOMES (Accepted) says an explanation's leaves are input facts with document path and line. The template's gates also consume records supplied from outside, such as test results, which have no document location. Under the current wording such an outcome could never be explained. I amended the requirement so that a leaf from a supplied input names that input. Confirm?" Answer: "Leaf names the input (Recommended) — Keep my decision: an explanation may end in a supplied record and names the input it came from. The outcomes users most need explained, the release gates, can then be explained." Not chosen: keeping the old wording.

### Decided by the orchestrator, continued

- 2026-11-16, mara-wamxz3: the explanation checks that no admitted program can reach (a proof that is not for the requested row, a reply that is not JSON, an aborted provenance run) are verified by inspection when mara-bx82k0 is reviewed. Rests on ADR-DEBUG-PAUSE-POINT, which keeps test-only hooks to the writer. Undo: a debug-only hook, which needs the maintainer's approval.
- 2026-11-16, mara-wamxz3: REQ-RUN-WITHOUT-TOOLCHAIN gets its Verification item in mara-haczya (0.4.0), not in alpha.3. Rests on the requirement being about the npm package, which mara-a4m588 delivers. Undo: add the item in alpha.3.
- 2026-11-16: mara-wamxz3 was started before the alpha.2 epic closed, since it writes documents only. Undo: not needed.

### Decided by the orchestrator, 2026-11-16, alpha.3 work

- mara-wrbjte: VER-ADMITTED-LANGUAGE establishes that an admitted program compiles (exits 0 with no compile diagnostic), not that a run prints nothing; four admitted fixtures print runtime warnings, which are faults and belong to VER-PROGRAM-FAULTS. Rests on REQ-PROGRAMS-ONLY-COMPUTE and REQ-PROGRAM-FAULTS-REPORTED. Undo: restore the empty-stderr criterion.
- mara-wrbjte: two findings the reviewer rated exotic were fixed anyway, because the parser's input comes from checkouts the user did not write: component expansion counts inherited bodies before expanding them, and every recursive step counts against one depth limit, so no program text exhausts memory or the stack. Rests on RISK-SOUFFLE-REACHES-OUTSIDE. Left to the backlog: quadratic resolution of very large forward-declared unions, and repeated inheritance colons that Soufflé accepts (mara-gz2qp5). Undo: pull the issue.
- mara-wrbjte: mara-stpjjc (compile the admitted fixtures with the pinned Soufflé) and mara-x0qc0q (Soufflé names an included file by its base name) were folded into mara-bx82k0. Undo: not needed.
- mara-y4asgm: left to the backlog: a line number above 2,147,483,647 is clamped in the exported facts (mara-n0q4k3). Undo: pull the issue.
- mara-msymag: a heading followed by an item table is an item wherever it stands, so a plain rendered example of an item on a conventions page is reported and `project fix` makes it an item; examples of items belong in a fenced code block. Rests on DES-REGIONS, DES-ITEM-STRUCTURE and REQ-REPORT-EVERY-PROBLEM: the other reading leaves real hand-written items silent. An item's table is recognized by a `Type` row plus an `ID` or `Status` row of any value; a custom HTML anchor before the heading is reported and split off, and fix adds no anchor next to it. Undo: a new issue that adds an explicit marker for examples.
- mara-bx82k0 was split into three issues with one outcome each: execution and invariants (mara-bx82k0), the query operations (mara-gfb6n6) and explanations (mara-jm5ecb). mara-tx0fcf now also waits for mara-gfb6n6. Undo: not needed.
- alpha.3 work was started beside the last alpha.2 issues, and mara-vt2ktq, mara-wrbjte and mara-y4asgm ran in parallel. Undo: not needed.

- 2026-11-16, mara-bx82k0 (review): an entry that names a relation its program does not declare, or one that cannot cross the boundary (`undeclared-relation`, `invalid-relation`), does not make `item list`, `project fix` and the other operations that evaluate nothing refuse the project. Validation reports it, and the operations that evaluate the entry fail on it, as on a compile error. Rests on DES-RULE-CONSUMERS (these diagnostics exist only against the admitted program) and on VER-WORK-COUNTERS (a request that evaluates nothing does no program work). Undo: have every operation admit the declared programs before it runs.
- 2026-11-16, mara-bx82k0 (review): two findings that need an unusual program (a compile error's caret after an `.include` on the same line; an invalid entry name hiding the table's other errors) are fixed only if small, otherwise they go to the backlog. An include cycle that hid a compile error is fixed, because validation said "valid". Undo: pull the backlog issues.

- 2026-11-16, mara-gfb6n6 (review): a gate request on a project whose configuration or schema has an error is refused as `invalid-project` with exit status 1 and no gate or item outcome; "unavailable" is only the outcome of a gate whose evaluation failed. Rests on DES-PROJECT-CONFIGURATION (every operation except validation refuses such a project) and REQ-PROGRAM-FAULTS-REPORTED (a refusal is no passing outcome); DES-QUERY-RESULTS says so. Undo: report the gate and every named item as unavailable there.

- 2026-11-16, mara-2dnzjc (review): crash records where the core pattern names a socket are not covered now; backlog issue mara-8dtsks holds the options (ignore SIGXFSZ and read the output limit from file sizes; a non-dumpable Soufflé through a build patch; a cgroup limit). The 1-byte soft core limit covers piped handlers and file patterns, which is this machine's configuration. Undo: pull mara-8dtsks.

- 2026-11-16: mara-ptq6xc (the Linux sandbox around Soufflé, a release-slice issue) was started on a free worker slot before the earlier slices closed; it needs only the evaluation of mara-bx82k0. Undo: not needed.

- 2026-11-16, mara-tx0fcf (review): a Verification or Validation with a counting `pass` passes its gate although the result source is not marked complete; completeness decides only when no result counts (`pending` when complete, `unavailable` when not). Rests on DES-TEMPLATE-INPUTS and DES-TEMPLATE-GATES as accepted. For the maintainer's attention: a source that is not complete could lack a later failing result for the same revisions, so a stricter gate would report unavailable there. Undo: change DES-TEMPLATE-GATES and the programs so that a pass needs a complete source.

- Maintainer, 2026-11-20 (mara-ptq6xc): without Landlock no rule program is evaluated, with no opt-out (ADR-SANDBOX-REQUIRED confirmed with the question tool). Known places where Landlock is refused: Docker's default profile up to 20.10.7, gVisor.
- Orchestrator, 2026-11-20 (mara-ptq6xc, provisional): the sandbox may allow libraries only in the loader's directory if Mara only ever runs the Soufflé it ships or builds; the limit is stated in DES-SOUFFLE-SANDBOX and wider layouts (RPATH, Nix with nix-ld, Homebrew on Linux) go to the backlog. If a user's own Soufflé is a supported setup, the worker resolves the real library directories instead. Rests on DES-PACKAGES and ADR-SANDBOX-REQUIRED. Undo: do the backlog issue.

## Decisions made by workers in alpha.3

Copied from each issue's closing summary when the epic mara-17tdb3 closed on 2026-11-20. mara-2dnzjc and mara-ptq6xc are not children of the epic; their work changes rule evaluation, so their lists are here too.

### mara-wamxz3 Write the Verification items for the alpha.3 requirements


- Native only in alpha.3: the rows that compare the browser's Soufflé start with mara-xsm8w6 (alpha.6), which builds it, as VER-HOST-COMPARISON already schedules the browser. mara-bx82k0 and mara-y4asgm asked for WASM tests, but no issue of this slice builds Soufflé with Emscripten.
- VER-EVALUATION-TIME-LIMIT is Draft: its requirement is Draft (provisional number), and the strict `Verifies` lifecycle forbids an accepted Verification of a draft requirement. The two never-ending programs of EVD-SOUFFLE-PROGRAM-REACH pass when stopped by the time or the memory limit, whichever comes first.
- Fault reporting (DES-RULE-EXECUTION): an error diagnostic at the project path of the program file (never the private copy) and line, message ending with the source line and a caret; a fault without a line, or from a limit, at the configuration line that declares the program and naming the limit; `project validate` invalid, `query execute` and `query view` fail, a gate and every item it names unavailable. Gate exit statuses are left to mara-bx82k0.
- The private directory is removed when the evaluation ends, so the reach cases can require that no file under the test's temporary directory, `TMPDIR` included, changed.
- An input that the consumer does not list is refused, as an undeclared argument is (DES-REQUEST-INPUTS).
- A leaf from a supplied input names the input instead of a document location (DES-RULE-EXECUTION).
- Cold counters: `project validate` runs each declared program once; `query execute`, `view` and `gate` once; `query report` once in provenance mode; `query list` and refused requests never. In CLI direct mode subprocess spawns equal program runs, which also shows that Mara runs no tests.
- The harness's gate case supplies a complete `verification_result` with one `pass` row per `Verifies` edge, without matching revisions; the report case explains a failing row of that gate. A program written in the test stands in until mara-tx0fcf.
- The libfunctors cases build the library with `cc` in the test: a C compiler is present wherever Rust links. `mara` itself runs without one on `PATH`.
- The review probes and the rewritten trial programs are copied into the repository as fixtures, with the rows the probes recorded natively, which serve as expected values for the admitted arithmetic and text functions.
- Decided by the orchestrator: the explanation checks that no admitted program is known to reach (proof for another row, a reply that is not JSON, an aborted provenance run) are verified by inspection when mara-bx82k0 is reviewed, as VER-EXPLANATIONS says. Rests on ADR-DEBUG-PAUSE-POINT, which keeps test-only hooks to the writer.
- Decided by the maintainer (review): REQ-EXPLAIN-OUTCOMES now allows a leaf that names the supplied input that brought its fact, instead of a document path and line. Reason: the `engineering` gates consume supplied results (REQ-EXTERNAL-RECORDS-AS-SUPPLIED), so under the old wording the outcomes users most need explained could never be explained, against NEED-CONFORMANCE-FEEDBACK. Question: "Provisional decision 3 of 3, nothing waits on your answer. REQ-EXPLAIN-OUTCOMES (Accepted) says an explanation's leaves are input facts with document path and line. The template's gates also consume records supplied from outside, such as test results, which have no document location. Under the current wording such an outcome could never be explained. I amended the requirement so that a leaf from a supplied input names that input. Confirm?" Chosen option: "Leaf names the input (Recommended) — Keep my decision: an explanation may end in a supplied record and names the input it came from. The outcomes users most need explained, the release gates, can then be explained." Not chosen: "Keep the old wording — An outcome whose explanation would end in a supplied record is reported as "cannot explain". The requirement stays as written; gates that use test results get no explanation." I decided what such a leaf shows: the input's name and the row's values as supplied, not a position in the request, since Soufflé keeps one copy of duplicate rows (DES-RULE-EXECUTION, VER-EXPLANATIONS).
- REQ-RUN-WITHOUT-TOOLCHAIN has no Verification in this slice, confirmed by the orchestrator: it is about `npx @convesoft/mara@<version>`, which 0.4.0 delivers (mara-a4m588); alpha.3 builds only the Soufflé it needs. mara-haczya now names it and requires rule evaluation with the packaged Soufflé.
- VER-FACT-ROWS copies `edge`, `folded`, `source_complete` and supplied inputs into relations headed by an item's MID (edge by its source; folded joined to the title or field value it folds; the others joined to every item), as DES-RULE-CONSUMERS requires; the long, memory and output-size programs of docs/verification-programs.mara.md give one row per item for the same reason.

### mara-vt2ktq Build the pinned Soufflé with a script


- The binary lives at `target/souffle-<version>/souffle` in the workspace, with a fixed path and no environment override. A new version gets a new directory, so a stale binary is never reused. Rejected: `$TMPDIR`, which is cleaned after 7 days and is specific to one machine; an override variable, which no consumer needs yet.
- The source is GitHub's tag archive `archive/refs/tags/2.5.tar.gz`, pinned by SHA-256 `5d009ad6…385067`. Its `src/` is identical to the trial's tree apart from the Emscripten patch.
- `PACKAGE_VERSION=2.5` is passed so that `souffle --version` names the release. The trial's binary printed an empty version.
- The smoke run is part of the script, not a cargo test, so the local checks keep working where the script has not run.

### mara-y4asgm Fact bridge: mara.dl, JSON facts and request inputs


- `item` holds the first `Type`, `ID`, `Status` values, an absent one as an empty symbol; status and type as schema names.
- `field` holds rows whose key the schema declares as a field of the item's type; enum values as schema names, text as written.
- `edge` holds only relation rows linking to a MID; mentions are no edges.
- `folded` = lowercase as search does, then NFC (not full Unicode case folding: `ß` stays).
- Facts JSON is rendered once per snapshot and kept with it; each run writes it into its private directory.
- An inline value is one row of one column, a decimal integer for a number column (no `+`); `@file` is relative to the command's directory.
- An input supplied twice, or `--complete` of an input the entry does not list, is refused naming it.
- Request-input checks live in the engine so every host checks alike.

### mara-wrbjte Parser for the admitted program language


Recorded in DES-PROGRAM-PROFILE (docs/rules-facts.mara.md), from Soufflé behaviour observed with the spike's static binary:

- `#` only inside comments and strings (Soufflé ignores it there); comments do not nest; `///` and `/** */` only right before a declaration, fact or rule; `//!` and `/*!` refused.
- Strings on one line with the escapes `\"`, `\\`, `\'`, `\n`, `\t`, `\r`; text UTF-8, non-ASCII only in strings and comments, no control characters but tab, LF, CR.
- Integers decimal, `0x`, `0b` within signed 32 bits, 2^31 only right after a unary `-`; IP-address literals refused.
- No Soufflé keyword, built-in type name, `?` or leading `__` as a name (so `__LINE__` is refused too).
- `.include` only between items outside components, its path relative without `.`/`..`, resolved against the including file's directory (as Soufflé does first), else `mara.dl` to Mara's declarations; a file included twice starts with `.once`, which comes first in its file; a declared program at project path `mara.dl` refused.
- Aggregates: `count :` or `sum`/`min`/`max` of one value, no `;` in the body (Soufflé reports that without a location); `min`/`max` only of values the aggregate's own atoms or comparisons show to be numbers.
- `<`, `<=`, `>`, `>=` refused on records (Soufflé refuses at compile time; needed so every admitted probe compiles).
- `.plan` only after a recursive rule with one head, no `;` and no parenthesised terms, versions below its number of recursive atoms, each order a permutation of its atoms (needed so `plan_bad` is refused).
- A boundary relation is also not `inline` (Soufflé cannot read or write it).
- Components resolve names in the instance, then enclosing instances, then the program; a type parameter only in its own component's body.
- Limits: nesting 100 deep (refuses the 300-deep `deep_expression` probe); 100,000 declarations, clauses and instances after expansion.

### mara-bx82k0 Evaluate programs: execution, invariants in validation, limits and faults


- Soufflé is found beside the `mara` binary, else at `../souffle-2.5/souffle` from it (Cargo's `target/`), never on `PATH`; packaging stays with mara-a4m588 (DES-SOUFFLE-BUILD).
- Limits: memory 1 GiB of address space (`RLIMIT_AS`, Soufflé fails with `std::bad_alloc`), output 64 MiB per written file (`RLIMIT_FSIZE`, `SIGXFSZ`), no core file; fixed, not configurable. Time limit `[rules] time_limit_seconds`, 1–3600, default 10 (DES-RULE-EXECUTION, DES-RULE-CONSUMERS).
- Base-name mapping: private copies are named `p<n>.dl` and include paths in them are rewritten to the copy the parser resolved, so each diagnostic names one file and Soufflé reads exactly what the parser read.
- A Soufflé compile warning (such as a relation without rules or facts) is a compile error (`program-error`): DES-DIAGNOSTICS allows warnings only from invariants. Codes: `program-refused`, `program-error`, `program-fault`.
- What `project validate` runs: each declared file that an entry names or that no other declared file includes, once; with facts when an invariant uses it, else against empty facts. `item validate` runs the programs invariants use, with every consumer of each program shaping the run, and reports their faults as the item's. Validation supplies each listed input empty and no `source_complete` row.
- Entries: names `[a-z][a-z0-9_-]*`, unique across the four tables; `inputs` optional, none on invariants; an invariant's `description` defaults to its `message`; gate `subjects` and `conditions` non-empty, `unavailable` optional; entry tables without `[rules]` are `missing-key`; a missing entry program is `io-error` at its line. A row about no item's MID is reported at the relation's configuration line.
- Invariant names become diagnostic codes through `diagnostic::intern`, so `Diagnostic::code` stays `&'static str`.

### mara-gfb6n6 Query operations: execute, view, gate and list in CLI and MCP


- Result format (DES-QUERY-RESULTS): `query` head with `columns`; each row with its `item` node or `null`; gate head `name`/`outcome`, items with `outcome` and the rows that caused it (`relation`, `role`, `values`); outcomes `passed`, `failed`, `unavailable`, `not-applicable`. Results page like reads; a gate's `diagnostics` hold the problems that made it unavailable first.
- `query view` returns the same JSON as `query execute`; only its human form differs (a Markdown table). Rejected: a `table` member in the JSON, which doubles each page's bytes.
- Items are named with the repeatable `--id`, as for `search` and `item list`; an unknown or ambiguous one refuses the request. A gate without `--id` is `invalid-argument` (exit 2) before anything is read.
- An operation runs only entries of its kind; another kind's name is `unknown-entry`, naming the operation that runs it.
- `query execute`/`query view` with a program that gives no rows fail with `program-failed`; a gate is unavailable with exit status 1. A gate exits 0 only when passed.
- MCP `input` is an object (name → string or list of rows); a string is always one value, never a file. Rejected: an array of `<name>=<value>` strings, which cannot pass rows.
- An `--input` file counts as a file read and is not hashed: it is no project input.
- `query list` reads the configuration and schema only and refuses an invalid project, like `schema list`.

### mara-2dnzjc Evaluation limits and limit tests leave crash records (systemd-coredump) on Linux


- The mechanism for both limits is a core limit of 1 byte (ADR-LIMIT-STOPS-NOT-CRASHES). `RLIMIT_AS` and `RLIMIT_FSIZE` stay as the limits, so no limit was loosened and no fallback is needed: the kernel has this check on every Linux, containers included. Rejected: a cgroup `memory.max` through `systemd-run --user --scope`, a cgroup that Mara creates itself, polling the child's memory, patching Soufflé, and `SIGXFSZ` ignored with size detection.
- The interrupted-write tests keep the default-action ("Killed") case and only add the core limit.

Rejected approaches: `systemd-run --user --scope -p MemoryMax=1G -p MemorySwapMax=0` works in this session (OOM kill by SIGKILL, about 4 ms each), but it needs `XDG_RUNTIME_DIR` and adds `INVOCATION_ID` to Soufflé's environment. The programs test harness also clears the environment, and the tests would use the user's systemd manager.

### mara-tx0fcf Engineering template programs


- One invariant per obligation of a rule, so each message names the fix: `requirement-has-basis`, `requirement-verified`, `design-has-basis`, `risk-assessed`, `risk-threatens`, `verification-has-method`, `verification-has-target`, `validation-has-method`, `validation-has-target`, `term-unique`, `term-aliases`, `links-to-accepted`, `supersedes-retired` (DES-TEMPLATE-ACCEPTANCE, DES-TEMPLATE-RELATION-LIFECYCLE). Each relation has one row per item, so no item gets the same diagnostic twice.
- An empty alias cannot reach a program (malformed row), so `term-aliases` checks only an alias equal to the title, folded. A link to no item is left to `broken-link`.
- Program split and names: `knowledge.dl`, `readiness.dl`, `risk-matrix.dl`; levels `low`, `medium`, `high`; a risk with two values of a field has no level (DES-TEMPLATE-QUERIES, DES-TEMPLATE-RISK-LEVEL).
- Inputs use item IDs; a result other than the four counts as `error`; with more than one `candidate` row, or more than one context for a Validation, no result for it counts; `pending` ranks worse than `unavailable`; the `validation-status` summary row has empty Validation, status and context, `not-validated` without an accepted Validation (DES-TEMPLATE-INPUTS).
- `implemented` reads a `code_link` relation with no rule body that can hold (`:- false`) until mara-hk2arx supplies code links; `source_complete("code")` is never present, so `verified`/`release-ready` are unavailable for every requirement without a failure (DES-TEMPLATE-QUERIES). A Validation can pass `release-ready`.
- Gate relations: subjects `requirement_subject`, `validation_subject`; conditions `not_accepted`, `breaks_rule` (names the invariant relation), `blocking_risk`, `unverified_requirement`, `not_implemented`, `verification_not_passed`, `validation_without_target`, `validation_without_context`, `validation_not_passed`; unavailable `risk_level_unavailable`, `code_unavailable`, `result_unavailable`, `validation_result_unavailable`.
- View cells and rows (DES-TEMPLATE-VIEWS): implementation `direct`/`design`/`missing`/`unavailable`; verification the worst accepted result or `none`; risks `blocking`/`unavailable`/`clear`; a requirement without a Verification has one placeholder row; `risk-treatment` has a row per threatened node.
- The `engineering` configuration is the documented example plus `[rules]` and the entries (DES-PROJECT-INIT); the corpus switched to it as DES-PERFORMANCE-HARNESS foresaw.
- VER-WORK-COUNTERS' validation row said "1 program run per declared program"; DES-RULE-EXECUTION runs only files that an entry names or no other file includes, so an included program (the risk matrix) runs within its includer. The row now says so.

### mara-jm5ecb Explanations: query report


- Request: `query report <name> --id <item>`, with exactly one item, plus the gate's inputs. It explains every row that makes the entry's outcome for that item: an invariant's rows about the item (outcome `failed` with rows, `passed` without), and a gate's rows that `query gate` lists for the item, with its outcome. Result `report` (`name`, `kind`, `item`, `outcome`) and `rows` (`relation`, `role` for gates, `values`, `proof` or `unexplained`). Exit 0 only when every row is explained. Recorded in DES-QUERY-RESULTS.
- The outcome comes from the provenance run's own outputs, read at Soufflé's first reply (readiness probe `explain __mara_ready()`). That keeps the cold count at 1 run, as VER-WORK-COUNTERS requires, and the report does not share the cache of normal evaluations (DES-RULE-EXECUTION).
- A fault before the first reply is reported as the evaluation's fault: outcome `unavailable`, no rows, diagnostics as for a gate. This covers the time-limit case of VER-EVALUATION-TIME-LIMIT, whose fault diagnostic names the limit. After the first reply, rows explained before an abort keep their proofs.
- Proof node format: derived (`relation`, `atom`, `rule` on one line, `children`), fact (`relation`, `values`, `path`, `line`), input (`input`, `values`), negated literal (`absent`). `source_complete` leaves are input leaves named `source_complete`.
- Leaf locations: `title`, `revision` and `field` at the item's line; `folded` at the first item in snapshot order whose title or field value is the text.
- Negated literals with a wildcard are shown as the rule writes them, because Soufflé prints arbitrary values for wildcards (observed: `item("M", "m,r", "m,r", "draft", "m,r", 0)`). Soufflé lists a step's atoms, then its negations, then its constraints; negations are paired with the rule's negated literals by order.
- Aggregates: any step whose rule holds `: {` outside strings, or any leaf of a relation that is no input, cannot be explained. This also catches aggregates that Soufflé shows only as a constraint such as `5 = 5`.
- Bounds: 16 MiB of replies per run; 1,000 steps and 32 KiB per proof (half a page), so that a record fits a page.
- Guide wording: the unbound-server note is shorter ("the project root, else found from where the server started. Pass it for another checkout or worktree"), and the validate bullet drops "before you finish".

### mara-ptq6xc Run the souffle child process in an operating-system sandbox on Linux


- Landlock plus a seccomp filter, through `libc` rather than the `landlock` crate; read access beneath the interpreter's directory rather than the exact libraries (ADR-SOUFFLE-SANDBOX).
- Without Landlock, Mara evaluates no program; ABI 1 suffices because the filter covers its gaps (ADR-SANDBOX-REQUIRED). Rejected: evaluating unsandboxed with or without a warning, requiring ABI 6/9/10, an opt-out option.
- Soufflé writes only beneath `out/`; the rest of the private directory is read-only to it.
- A planted output that Soufflé cannot open stays a passing result with the program's rows (Soufflé 2.5 skips it silently); the write is stopped. Follow-up: mara-k4g6y1 (backlog).
- Residual, documented in DES-SOUFFLE-SANDBOX: metadata reads outside (`stat`, `access`), and code mapped from memory running inside the same sandbox. Untested here: arm64 (no target installed); its system call constants were checked in `libc`.

### Decided by the orchestrator when the epic closed

- Orchestrator, 2026-11-20: mara-zp38s3 closed as a tracker change, without a worker: mara-jm5ecb's template tests already require a proof for every failing invariant row and every failed or unavailable gate item (VER-TEMPLATE-CASES, 'has an explanation'). Undo: reopen it and add the per-row leaf assertion.
