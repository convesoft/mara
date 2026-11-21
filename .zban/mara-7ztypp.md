---
title: Review the decisions workers made in alpha.5
status: todo
type: task
tags:
  - maintainer
blocked_by:
  - mara-779tms
created_at: 2026-11-14T11:14:21Z
---

Goal: the maintainer reviews what workers decided on their own in the alpha.5 slice.

Context: AGENTS.md (Deciding during the rewrite). When the alpha.5 epic (mara-779tms) closes, the orchestrator appends the `## Decisions made` list of each of its issues below. This issue blocks the release workflow, not the next slice. No worker starts it.

- [ ] Every listed decision accepted, or overturned by a follow-up issue

## Decided by the orchestrator

One line per decision made on the maintainer's behalf under the rule of 2026-11-16: what was decided, what it rests on, how to undo it. "Provisional" marks a decision that is the maintainer's to confirm and stands until they answer.

- 2026-11-16, mara-ttvp9z: inline marker comments stay supported (`check(); // @mara implements REQ-CHECK`), as in 0.3.0; the worker's own-line rule would have dropped them. Rests on REQ-CODE-MARKER-LINKS and AGENTS.md (0.3.0 where the capability is kept). Undo: not needed; dropping them would be the maintainer's decision.
- 2026-11-16, mara-ttvp9z: a missing grammar is the error `unavailable-grammar`, so `project validate` fails on a fresh machine until `mara code add` has run. Rests on DES-DIAGNOSTICS and REQ-ONLY-CODE-ADD-DOWNLOADS. Undo: allow a warning for this one case.
- 2026-11-16, mara-ttvp9z: a project with no code language fails the template's `verified` gate instead of reporting it unavailable. Rests on DES-CODE-READERS (code counts as completely read) and the gate's design. Undo: change the gate design.
- 2026-11-16, provisional, mara-ttvp9z: ADR-GRAMMAR-TRUST-ON-FIRST-USE (Draft): grammars in the built-in catalog are checked against built-in checksums; a repository the user names is trusted on first use, its fetched bytes and repository are pinned, and every later fetch and load is checked against the pin; transport is HTTPS, or HTTP only to the local machine. No Accepted item covered this before; it weakens none. A downloaded grammar is a `.wasm` file run inside tree-sitter's WebAssembly runtime in Mara's process, without access to files, the network or native code; no CPU or memory limit is set for it. Undo: require a checksum argument for every repository outside the catalog.
- 2026-11-16: mara-ttvp9z was started before the alpha.4 epic closed, and mara-yhxq5x was split (mara-hk2arx for programs and the template's code cases, mara-9fphtg for the daemon) and started beside alpha.3 and alpha.4. Undo: not needed.

- 2026-11-16, confirmed by the maintainer: asked how Mara should trust a grammar from a repository the user names that is not in the catalog (options: trust on first use; require a checksum; catalog only), the maintainer chose "Trust on first use (Recommended)". ADR-GRAMMAR-TRUST-ON-FIRST-USE is no longer provisional; mara-h0hxa3 sets it to Accepted.

- Orchestrator, 2026-11-20 (mara-s9gak4): a pinned `.wasm` that is valid WebAssembly but no tree-sitter grammar aborts the process inside the tree-sitter runtime. Rated exotic (only a pin whose checksum names such a file reaches it; ADR-GRAMMAR-TRUST-ON-FIRST-USE): the worker fixes it if small, otherwise states the limit in DES-GRAMMAR-ASSETS and files a backlog issue. Undo: do that backlog issue.
- Orchestrator, 2026-11-20: mara-hk2arx is started before mara-jm5ecb and mara-s9gak4 are integrated; it rebases over both.

- Orchestrator, 2026-11-21 (mara-gqvffn, review): a grammar load that ends after its 10 s limit keeps what it loaded: its thread parks and never tears its WebAssembly code down, as for a parse that does not answer. The worker had filed it as backlog (mara-dn4bkp); the reviewer judged that a loaded machine reaches the crash at exit, and the fix is two lines, so it is done in mara-gqvffn. Rests on ADR-GRAMMAR-WATCHDOG-THREAD and ADR-LIMIT-STOPS-NOT-CRASHES. Undo: let the late thread drop what it loaded and make exit safe another way.
- Orchestrator, 2026-11-21 (mara-0x04mn): the agent guide gets a short code-links section, within VER-AGENT-GUIDE's 3,800 characters. Rests on REQ-CODE-MARKER-LINKS and DES-CODE-MARKERS. Undo: remove the section.

- Orchestrator, 2026-11-21 (mara-h0hxa3, review): decisions given to the worker for `mara code add`. (a) Every check of the configuration that can run before the fetch runs before it; after a later refusal the verified files may stay in the per-user cache, which is addressed by checksum, and the diagnostic says so; the project's files stay untouched (REQ-REFUSED-EDIT-CHANGES-NOTHING). (b) Provisional, for the maintainer: when the environment names a proxy the client cannot use (for example `ALL_PROXY=socks5://`), `code add` fails with `fetch-failed` and connects to nothing; no SOCKS support is added. Undo: enable the client's SOCKS feature. (c) The pin's string is written by the `toml` crate (`display` feature), not by an encoder of Mara's own. (d) The row of VER-CODE-ADD about `unavailable-grammar` is narrowed to operations whose results carry diagnostics; the result of `item delete` gains no field (closes mara-9r4749). Undo: add `diagnostics` to that result. (e) A redirect target must be `https://` and may carry a query; the limits on credentials, query and fragment apply to the URL the user or the catalog gives. All rest on REQ-ONLY-CODE-ADD-DOWNLOADS, DES-GRAMMAR-ASSETS and ADR-GRAMMAR-TRUST-ON-FIRST-USE.
- Orchestrator, 2026-11-21 (mara-h0hxa3): waiting on the maintainer: what the shipped catalog contains. Until then it is empty and `code add <language>` without `--repository` is refused with `unknown-grammar`. Recommended: rust 0.24.2 and python 0.25.0, the releases the fixtures test, once the maintainer confirms their URLs.

- Orchestrator, 2026-11-21 (mara-b76x4g, review): `get` of a code location builds no snapshot, so it returns the diagnostics of its own listing and scan and no longer the documents' (user-visible; follows from the issue's 0 parses; DES-READ-RESULTS, DES-CODE-READERS). DES-PAGING names the exception: the cursor of such a `get` is keyed from the code file's contents, or its scan's key, and the diagnostics it returns; a cursor issued before the change is stale when the documents had diagnostics then. Undo: build the snapshot again for a code `get`.

- Orchestrator, 2026-11-21 (mara-agdnwh, review and re-check): how a session forgets code files. A result that reuses a record the session no longer holds is not given back; only what a scan itself read again can enter a session that has no record of the path; and a scan drops only paths of the record it started from, so a file that a newer scan added stays. Chosen over generation counters for listings. Left open on purpose: a file that an older scan did read again, and that a newer listing forgot meanwhile, can be put back until the next scan drops it; it costs one record and one stat and never a stale answer. Rests on REQ-NEVER-STALE and DES-DAEMON-SESSIONS. Undo: count listings.

- Maintainer, 2026-11-21 (mara-yk8drm): the shipped grammar catalog holds rust 0.24.2 and python 0.25.0, from the tree-sitter release assets, with `tags.scm` from the same tag. Chosen over an empty catalog for 0.4.0 and over entries the maintainer names. This replaces the orchestrator's open question of the same day.

- Orchestrator, 2026-11-21 (mara-h4z424, review): a `get` of a path that a code include pattern matches and that fails as not found carries the diagnostics of the check on the way to the path (for example `unsupported-gitignore`), instead of none (user-visible). Rests on AGENTS.md (a failure reports a diagnostic the user can act on) and DES-CODE-READERS; it closed the worker's backlog issue mara-v1bnwd inside mara-h4z424. Undo: drop the diagnostics from that failure.
- Orchestrator, 2026-11-21 (mara-h4z424, re-check): integrated with one work-only leftover of the re-check (a `.gitignore` reread within 2 s of its edit compiles the same rules again) and three of the review; all four are in the backlog issue mara-yv42pq. None gives a stale answer. Rests on REQ-NEVER-STALE holding and on the review budget. Undo: do mara-yv42pq.
- Orchestrator, 2026-11-21 (mara-0t6mpc, review): the reviewer's P2 (the warm test of `get` of a document awaits no watcher report) was dropped: the test changes no file after the daemon starts, so nothing in it depends on the watcher; `served` proves the daemon answered. Undo: not needed.
- Orchestrator, 2026-11-21 (mara-7kfxn1, review): two debug-only changes of the test hooks stand: an empty value counts as unset for every hook, and the evaluation and watcher pauses publish `paused` by rename. Rests on ADR-DEBUG-PAUSE-POINT. Undo: not needed; no release build holds a hook.
- Orchestrator, 2026-11-21: the alpha.5 issues that share files ran in the order mara-0t6mpc, mara-h4z424, then mara-9fphtg after mara-7kfxn1, by `blocked_by` edges added at `94e116d`. Undo: not needed.

- Orchestrator, 2026-11-21, provisional (mara-cp1skx): the epic's "Done when this repository's code links to the requirements it implements" had no issue; mara-cp1skx was filed for it under the epic. This repository declares Rust at the basic level, without a pinned grammar, so that `project validate` in the local checks needs no download; markers are for requirements only. Rests on the epic's outcome, REQ-CODE-MARKER-LINKS and REQ-ONLY-CODE-ADD-DOWNLOADS. Undo: run `mara code add rust` in the repository (declaration level; the markers stay), and say in AGENTS.md that a fresh machine runs it before the local checks.

- Orchestrator, 2026-11-21 (mara-9fphtg, review and re-check): of the review's verification gaps, the daemon cases the checklist names were added (code readers in the host comparison, grammar reuse, `served` in the freshness test, modes and mtimes in the comparison with direct mode); the rename's conflict, interruption and retry cases through the daemon were left out, because VER-CODE-EDITS asks the rename through the daemon and not each case (backlog mara-djyqxf). The worker's finding that a directory only code can be in is watched only between a request that lists code and the next derivation that lists none stands as DES-DAEMON-SESSIONS now states it: no answer can be stale, since every request checks the files on disk (backlog mara-enc837 asks for one rule). Undo: do the two backlog issues.

- Orchestrator, 2026-11-21 (mara-cp1skx, review and re-check): where this repository's markers go. A marker is on each function or type that carries out a distinct obligation of a built Accepted requirement, for the hosts that exist; the code that performs the behaviour is preferred to a type that holds its data; a declaration called directly from a marked one needs none; a requirement that no single place or only `.mara/rules/*.dl` implements carries none. The orchestrator's first rule (one marker per requirement) was too narrow and was replaced after the review. The convention is the line in AGENTS.md; no Mara item holds it, because it is this repository's convention and not the product's. Result at `b561961`: 94 markers on 76 declarations, 43 of 51 Accepted requirements. Undo: change the AGENTS.md line and the markers.
- Orchestrator, 2026-11-21 (mara-cp1skx): eight Accepted requirements have no code link: four are not built yet (REQ-NODE-LIBRARY, REQ-BROWSER-LOCAL-WORKSPACE, REQ-BROWSER-ACCESS-AUTHORIZED in alpha.6; REQ-RUN-WITHOUT-TOOLCHAIN in 0.4.0), two have no single place (REQ-NEAR-LINEAR-SCALING, REQ-READS-NEVER-WRITE; backlog mara-006gqt), two are built in Datalog (REQ-TEMPLATE-ACCEPTANCE-RULES, REQ-TEMPLATE-GATES-PROVE-READINESS; the maintainer's question mara-pmjt08). No exclude patterns are added to the product. Undo: do those two issues.
- Orchestrator, 2026-11-21: the epic's "Done when" was run at the tree of `b561961`: `query execute implemented --limit 100` lists 94 code links on 43 requirements, `project validate` reports 0 errors and the one known warning, and the seven local checks pass.

## Decisions made by workers in alpha.5

Appended by the orchestrator on 2026-11-21, when the epic mara-779tms closed: the `## Decisions made` sections of its issues and of mara-ttvp9z, which wrote the slice's Verification items, as their closing summaries hold them.

### mara-ttvp9z Write the Verification items for the alpha.5 requirements

- Marker form (DES-CODE-MARKERS), changed by the review (finding 3): a marker is a comment whose text is the marker, on its own line or after code on the same line, as in 0.3.0: the comment start, `@mara`, a forward relation name and an ID, and nothing after them but white space. `@mara` later in a comment's text is no marker. A JSDoc ` * @mara …` line works when `*` is a prefix.
- Comment start at the basic level (review, finding 3): the first configured prefix on a line that stands at the line's start or after a space or tab. So `"// @mara …"` holds no comment, while `" // @mara …"` is read as one and is `invalid-marker`, because text follows the ID. At grammar level a comment starts where its node starts, or at a prefix that begins a later line of a block comment, and text in a string is never a comment. An inline marker belongs to the innermost declaration that contains the start of its line, else to the file.
- `item rename` fails with `io-error` at a declared code file it cannot read, before writing anything (DES-CODE-READERS; review, finding 1).
- A prefix followed by `@mara` without one declared relation and one ID-form word is `invalid-marker`. An ID no item has is `unresolved-marker`, and a relation that does not lead from `code` to the target's type is `disallowed-target`. All three are errors at the marker, without an item ID or MID (DES-SCHEMA-CHECKS rows added).
- Code files are read as bytes, in LF-ended lines, so a file that is not UTF-8 still links and a rename changes only the ID bytes.
- At grammar level the same rule reads only lines inside comment nodes; a marker-like line in a string links at basic level only. The owning declaration follows 0.3.0: the declaration the comment precedes, else the innermost one that contains the marker, else the file. The location line is the marker's line at basic level, and at grammar level the declaration's first line, or 1 for the file. The source is always the marker's line.
- One code link per marker; a marker naming an ID that several items have links to each, as MID links do.
- `get` and `relation list` of `<path>:<line>` in a code file name the line at basic level and the owning declaration at grammar level; `<path>` names the whole file.
- `source_complete("code")` (DES-CODE-READERS): every declared code file read and scanned at either level, a grammar fallback included. A project with no code language has read all its code, so the engineering `verified` gate fails a requirement without an implementation instead of staying unavailable. An `io-error` code file leaves code incomplete. A program uses code links when it, or a file it includes, names the code-link relation.
- Grammars (DES-GRAMMAR-ASSETS): `code add` writes the configuration through the write procedure. A missing, mismatched or unloadable grammar is `unavailable-grammar`, an error at the pinning line that names `mara code add`, since DES-DIAGNOSTICS makes every built-in failure an error. A grammar runs as WebAssembly in tree-sitter's WASM store inside Mara's process, and `tags.scm` is a query.
- Provisional, decided by the orchestrator on the maintainer's behalf, for the maintainer to confirm: the grammar download trust policy, ADR-GRAMMAR-TRUST-ON-FIRST-USE (Draft) beside DES-GRAMMAR-ASSETS. Catalog entries carry their checksums. A repository the user gives is trusted on first use: the checksum of its fetched files and the repository are pinned, and every later fetch and load is checked against the pin. It must be `https://`, or `http://` only to a loopback host. Rejected: no pin; a required checksum argument for every repository outside the catalog; plain HTTP anywhere.
- Decided by the orchestrator on the maintainer's behalf: a missing grammar is the error `unavailable-grammar`, so `project validate` fails on a fresh machine until `mara code add` has run. Rests on DES-DIAGNOSTICS and REQ-ONLY-CODE-ADD-DOWNLOADS; the diagnostic names the fix. Undo: allow a warning for this one case.
- Decided by the orchestrator on the maintainer's behalf: a project with no code language fails the template's `verified` gate instead of reporting it unavailable. Rests on the gate's design: code counts as completely read, and nothing implements the requirement. Undo: change the gate design.
- Decided by the orchestrator on the maintainer's behalf (review, finding 3): inline marker comments stay supported, as the docs and then 0.3.0 order requires; dropping a capability that users see is not a worker's decision.
- `code add` is in neither rendering of the agent guide, like `prime` and `mcp` (DES-AGENT-GUIDE): it is no MCP tool.
- No read-back scan of rewritten code files: a rename replaces only an ASCII ID token inside a recognized marker line. VER-CODE-WORK-COUNTERS counts no code scan after the edit.
- "Reads no code" is asserted as the counts of the same case with no code language declared, since the counters do not tell code reads from document reads.
- Warm rows for code inputs live in VER-CODE-WORK-COUNTERS, to stay clear of mara-bx63yb's edits of VER-WORK-COUNTERS; that item's existing "One code file edited" row can only run from alpha.5.
- ABI mismatch is verified by inspection: no released grammar has an ABI outside the runtime's range, so a truncated module tests the same fallback. The real catalog is checked by an ignored test run before each release.

### mara-yhxq5x Basic-level comment markers

- Configuration syntax: `[code]` with `format_version` 1 (the marker syntax is a public format) and `[code.languages.<name>]` with `include` patterns instead of extensions, consistent with `[documents]`. Written in DES-CODE-MARKERS and DES-PROJECT-CONFIGURATION.
- A file matched by two languages belongs to the first in the configuration's order; a document or a write's temporary file is never a code file (DES-CODE-MARKERS, DES-WRITE-PROCEDURE).
- `@mara` may follow the comment prefix directly (`//@mara`), as 0.3.0 read it; where several prefixes start at the same place, the longest is the comment start (`///`). Limit stated: a prefix that is also an operator, such as `*`, starts a comment after a space, so `a * b; // @mara …` holds no marker (DES-CODE-MARKERS).
- A `disallowed-target` marker still links (as VER-CODE-FACTS expects); an unresolved or invalid one does not.
- `item rename` rewrites every marker that names the old ID, including one whose relation is undeclared; the edit replaces only the ID bytes and is checked instead of read back (`unreadable-edit` on a failed check) (DES-CODE-READERS).
- Code is incomplete also when a directory or `.gitignore` that may hide a code file cannot be read or applied; `item rename` then fails with `io-error` (DES-CODE-READERS).
- An item's code links follow its incoming links in path and line order; the code node is `{kind: "code", path, line, declaration}`, `line` null for a whole file (DES-READ-RESULTS).
- `get` of a code location lists code files without reading them and reads only that one (DES-CODE-READERS).
- `item validate`, `project fix`, `item move` and every other operation read no code, per DES-CODE-READERS' table.

### mara-s9gak4 Grammar-level endpoints

- The pin: `grammar = { version, checksum }` in the language's table; checksum = BLAKE3 of the BLAKE3 hashes of the `.wasm` and `tags.scm`; version restricted to letters, digits, `.-_+`, not starting with `.`, since it is a path component (DES-GRAMMAR-ASSETS).
- The cache: `$XDG_CACHE_HOME/mara/grammars` (absolute only), else `~/.cache/mara/grammars`, `~/Library/Caches/mara/grammars` on macOS; files at `<language>/<version>/<checksum>/grammar.wasm` and `tags.scm`; the tree-sitter name is the language's name with `-` as `_` (DES-GRAMMAR-ASSETS).
- Grammar files are not counted among input reads and hashes, so the `get` row holds as written; a host reloads a grammar only when its files' stats change (DES-GRAMMAR-ASSETS).
- Time and memory: 128 MiB (tree-sitter's store), 1 s + 1 s per 256 KiB per file, a watchdog at twice that plus 1 s that abandons the grammar for the process; wasmtime epochs and fuel cannot be used through tree-sitter's API (DES-GRAMMAR-ASSETS, ADR-GRAMMAR-WATCHDOG-THREAD).
- A file the grammar could not parse falls back alone with `unavailable-grammar` at the pin's line naming the file, keeping that diagnostic's documented place (DES-GRAMMAR-ASSETS, DES-SCHEMA-CHECKS).
- Attributes between a marker and its declaration are nodes of kinds `attribute_item`, `inner_attribute_item`, `decorator`, `annotation`, `marker_annotation`, `attribute_list`; a preceded declaration may also be the child of a node starting there on the same line (`export function`); "the start of its line" is its first text (DES-CODE-MARKERS).
- The owner of an arbitrary line follows the marker rules, and `get` of a line the file owns returns the whole file; `get <path>` also scans the file at the grammar level, so that it reports `unavailable-grammar` like every operation that reads code (DES-CODE-MARKERS, DES-CODE-READERS).
- Every file of a grammar language is parsed, not only those holding `@mara`; skipping the others is filed as backlog mara-jtjr3h.
- Test hooks in debug builds only, after ADR-DEBUG-PAUSE-POINT: `MARA_TEST_GRAMMAR_BUDGET_MS` and `MARA_TEST_GRAMMAR_STALL`, since no released grammar traps or loops.

### mara-hk2arx Code links in programs and the template's code cases

- `code_link(rel: Rel, dst: Mid, path: Path, line: number, marker_line: number, declaration: symbol)` is declared in `mara.dl`, still format version 1 (not released). At the basic level `line` is the marker line and `declaration` is empty, so the grammar level changes values, not columns (DES-FACT-BRIDGE).
- A program uses code links when a clause in its own files (not `mara.dl`) names `code_link` unqualified, or an entry of its run consumes `code_link` (DES-CODE-READERS).
- `query` operations and `item validate` list the code files with the documents, and read and scan them only after admitting the program and checking the inputs, and only if the program uses code links. Other runs get an empty `code_link` (Soufflé rejects a declared relation with no input) and no `source_complete("code")`.
- `item validate` reads code when an invariant's program uses code links, so its findings match `project validate`'s. Added as a row of DES-CODE-READERS.
- `source_complete("code")`: added when every configured code file was read, including when `[code]` is absent. An invalid, unresolved or disallowed marker does not stop it. An unreadable file, or a directory or `.gitignore` that may hide one, does. A request that reads no code adds none (DES-CODE-READERS).
- The template's `implemented` reads only `implements` code links (DES-TEMPLATE-QUERIES). Both copies of `readiness.dl` are identical.

- `query report` reads code exactly as `query execute` does, and a `code_link` proof leaf is a fact at the code file and the marker's line (DES-RULE-EXECUTION).
- The performance corpus pins one `implements` marker for the requirement that `query gate` judges, so its `verified` case passes at every size (DES-PERFORMANCE-HARNESS).

### mara-d005q9 Run the code-link case of VER-ONE-HOP

- None.

### mara-h0hxa3 mara code add

- Given repository layout: `<url>/<language>/<version>/grammar.wasm` and `tags.scm`, the cache layout without the checksum; `--version` is required with `--repository` and refused without it.
- A catalog entry holds two URLs (`.wasm` and `tags.scm`), since releases publish `tags.scm` only inside the source tarball.
- A language the configuration lacks is declared from its catalog entry (`include = ["**/*.<ext>"]`, its comment prefixes); with a repository only, it is refused with `unknown-grammar`.
- With a pin: no options fetch the pinned release (from the pin's repository, or the catalog when it has that version and checksum, else `unknown-grammar`); the same repository and version are checked against the pin; another repository or version is trusted on first use and replaces the pin. Cached files that match are not fetched again.
- Loopback means an IP literal in 127.0.0.0/8 or [::1]; `localhost` is refused. Credentials, query and fragment are refused.
- HTTPS follows at most 5 redirects, to https only, and honours the environment's proxy; loopback http follows no redirect or proxy. 64 MiB per file, 30 s to connect, 300 s per file. A `.wasm` without the WebAssembly magic is `fetch-failed`.
- New failure codes: `unknown-grammar`, `fetch-failed`, `checksum-mismatch`; a configuration with errors is `invalid-project`, one the pin cannot be written into in place is `unreadable-edit`.
- Result: `language`, `version`, `checksum`, `repository` (null for the catalog), `fetched`, `pinned`.

### mara-0x04mn Teach code markers in the agent guide

- The guide gets a separate `## Link code` section (orchestrator's answer) rather than bullets under Author; recorded in DES-AGENT-GUIDE, and VER-AGENT-GUIDE's section list extended to match.
- The section also says that markers count only in files of a `[code.languages.<name>]` table, since `project init` writes no `[code]` and markers elsewhere do nothing.

### mara-gqvffn Loading a grammar does not hold up other languages, and a load that times out is not tried again

- The load limit stays 10 s and a timed-out load is dropped for the process by language name and checksum, like a stalled parse; it ends with the process, and another checksum is another grammar (DES-GRAMMAR-ASSETS, ADR-GRAMMAR-WATCHDOG-THREAD).
- A request that was waiting for a load of the same grammar when it timed out gets `unavailable-grammar` with the same reason, without trying again (DES-GRAMMAR-ASSETS).
- A load that ends while the grammar is dropped (by a stalled parse of the grammar it replaced) is not used by that request (DES-GRAMMAR-ASSETS: later requests neither load nor wait).
- Test hooks `MARA_TEST_GRAMMAR_LOAD_STALL` and `MARA_TEST_GRAMMAR_LOAD_MS`, debug builds only, like `MARA_TEST_GRAMMAR_STALL` and `MARA_TEST_GRAMMAR_BUDGET_MS` (VER-GRAMMAR-MARKERS).

### mara-b76x4g get of a code path parses no document

- `get` of a code location tests the configured patterns before it lists anything, and then lists only where a code include pattern can match (DES-CODE-READERS). It reads no document or program and builds no snapshot.
- Its diagnostics are those of its own listing and of the scan, not the inventory's diagnostics of the documents. DES-READ-RESULTS asks for those only on a read of documents.
- A path that matches a code pattern but is not listed as code (ignored, missing, or below another project's root) falls back to the earlier document path. It reads each `.gitignore` that both walks visit once more than before, and fails as before.

Rejected: a check that visits only the path's ancestors. It would repeat `Walk`'s `.gitignore` and nested-root logic, and DES-CODE-READERS says `get` lists the code files.

### mara-agdnwh A warm code scan in a session copies nothing it does not change and forgets deleted files

- `get` forgets its contents and grammar-level scan of a code file when a listing of the code files no longer finds it as one: deleted, renamed, ignored, no longer matched, or now a document or a program. The listing may be any request's that lists code, or `get`'s own. Recorded in DES-DAEMON-SESSIONS. Rejected: forgetting only in a scan's give-back, which would keep paths that only `get` read until the session is dropped.
- The session's record of scanned files is pruned by the next scan, after it has reused scans by key, not by a listing, so a renamed unchanged file is not scanned again. Recorded in DES-DAEMON-SESSIONS. Rejected: pruning at the listing, which lost the renamed file's scan (review of `bd0991a`).
- `give_back` judges "read again" against the record the scan started from. It compares with the session's record at the moment it gives back only to skip a result the session already holds as it is. A file the scan read itself comes back even when a newer listing dropped it meanwhile, until the next scan: one record and one stat, no stale bytes, since each request checks membership and stats. Recorded in DES-DAEMON-SESSIONS. Rejected: generation counters for listings (orchestrator).
- Follow-up mara-cytrb4 (backlog): the write hand-over and `get`'s listing still copy inventory maps.

### mara-yk8drm Fill the shipped grammar catalog with Rust and Python

- The `tags.scm` URL is `raw.githubusercontent.com` at the release's tag: the maintainer named "the same tag of the repository", and DES-GRAMMAR-ASSETS needs a URL of the file alone. Written into DES-GRAMMAR-ASSETS.
- Extensions and comment prefixes, which the maintainer left open: rust `rs` with `//` and `///` (as the tests' own catalog does; `*` from the docs' configuration example left out, since it is also an operator), python `py` with `#`. Written into DES-GRAMMAR-ASSETS.

Follow-up: mara-gjn0mr, the module comment of `catalog.rs` (owned by mara-7kfxn1 during this issue) still calls the shipped catalog empty.

### mara-0t6mpc get of a document or an item lists no code

- DES-CODE-READERS: a document's path, a line in a document and a path that no code include pattern matches are read as `get` of an item is. Nothing is listed, watched or read for code alone, and no diagnostic of code is reported. A directory that holds documents and code is still listed and watched for its documents. A path that a code include pattern matches and that is no code file is read as a document after that listing.
- VER-CODE-WORK-COUNTERS gains a cold row and a warm row. The cold row: `get` of a document's path, of a line in it, or of a path that no code include pattern matches, with a `.gitignore` and a problem in the code tree, at the basic and at the grammar level, counts what `get` of the item by ID counts. The warm row: the same `get` counts 0 at either level, the session holding the snapshot and the inputs settled. Neither reports a diagnostic of code.
- Out of scope, left to mara-h4z424: an address that a code include pattern matches but that is no file (such as `src/missing.rs`) still needs a listing to decide that it is no code file. DES-CODE-READERS allows that listing. mara-h4z424 limits it to the path's ancestor directories.

### mara-7kfxn1 One mechanism for debug-only test hooks, and a release build that holds none

- The hooks live in `mara::hooks`, public but `#[doc(hidden)]`, only because the binary crate reads two hooks (`server.rs`, `catalog.rs`). `value`, `pause` and `append` are `pub(crate)`; the module exports only `Hook`, `path` and `number`. Recorded in ADR-DEBUG-PAUSE-POINT.
- The boundary is one function, `value`. Hook logic after it compiles in both builds, but every function of the module takes the hook and acts only on what `value` gives (`append` no longer takes a path), so a release build reaches none of it, from the CLI or from another crate, and holds none of the names. Recorded in ADR-DEBUG-PAUSE-POINT.
- An empty value counts as unset for every hook. This was already true for all of them except `MARA_TEST_GRAMMAR_EVENTS`, which used to take an empty value to mean the working directory. Recorded in ADR-DEBUG-PAUSE-POINT.
- The pause of an evaluation and of the watcher now also creates `paused` whole. When it can't create the file, it goes on without pausing, as before. Only the writer refuses the edit then. Recorded in ADR-DEBUG-PAUSE-POINT.
- The grammar load hold (wait while `hold` exists) stays in `grammar.rs`. It writes no marker, so it is not a pause point. Recorded in ADR-DEBUG-PAUSE-POINT.

### mara-h4z424 get of a code location checks only that file's directories and does not choose kept contents by time

- Which kept record answers is decided by stat: the record a check would keep (its stat is the file's and its check settled it), with no order of records. This covers code contents (from `get` or a scan) and directory listings and `.gitignore` files (from the inventory or from `get`). The choice is made where the walk reaches each one, after its parent admitted it. Written in DES-DAEMON-SESSIONS.
- `get` keeps its listings in a session map apart from the inventory, not watched. Every walk reuses and refreshes them, and a listing of the code files drops the directories it does not reach. Written in DES-DAEMON-SESSIONS.
- `get` forgets kept code only for its own path. Another file that left stays until the next code listing or a `get` of that path: it costs memory, never a stale answer. Written in DES-DAEMON-SESSIONS.
- No counter for directory listings or watches. VER-CODE-WORK-COUNTERS says a listing shows only where it reads a `.gitignore` or reports a problem.
- Decided by the orchestrator: a not-found `get` of a path that a code include pattern matches carries the diagnostics of the check on the way, after its own. Such a path is never a document or an ID, so that `get` always fails, and no success case needs a rule. Written in DES-CODE-READERS.

### mara-9fphtg Code links through the daemon

- VER-HOST-COMPARISON: the daemon case's project declares an invariant over code links (`readiness.dl`'s `not_implemented`, a warning), so that `item validate` reads code there.
- VER-CODE-WORK-COUNTERS, warm rename row: limited to written contents new to the session; contents a scan already holds under the same key are not scanned again.

- DES-DAEMON-SESSIONS: a listing of the code files keeps the listings and `.gitignore` records of every directory it lists apart from the inventory, so that the next listing of the code files reads no unchanged `.gitignore` again.
- DES-DAEMON-SESSIONS: a directory that only code files can be in is watched from a request that lists the code files until the next derivation that lists none. That follows the existing rule "those the last inventory listed"; whether to watch such directories steadily or never is mara-enc837.
- VER-CODE-WORK-COUNTERS, warm row "`item rename` that rewrites markers": the write scans no code, since it reads back no code file (DES-CODE-READERS). The next request that reads code scans each distinct code content written, once.
- VER-HOST-COMPARISON: the daemon case's project declares a code language and holds markers.

### mara-cp1skx Link this repository's code to the requirements it implements

- The repository's marker convention is the AGENTS.md line under "Mara documents": where markers go, which requirements carry none, and checking the `[code]` patterns when a file or directory is added under `crates/mara-engine/src/`.
- Include patterns list the engine's subdirectories so that `crates/mara-engine/src/code/tests.rs` stays out. Patterns have no exclude, and changing its test data would change tests. The TOML comment in `.mara/config.toml` records this.
- A marker is a plain `//` line directly above the declaration's doc comments and attributes. At the grammar level it then belongs to that declaration, so `mara code add rust` needs no change to the markers.
- REQ-EXTERNAL-RECORDS-AS-SUPPLIED and REQ-UNAVAILABLE-NOT-PARTIAL keep only their Rust parts. Telling a missing record from an unavailable one is in `.mara/rules/readiness.dl`, which cannot be marked.

- Maintainer, 2026-11-21 (mara-znydv4): this repository pins the Rust grammar (rust 0.24.2 of the shipped catalog) and scans every file under the two crates' `src`, asked in the orchestrator's thread with the words "Yes please pin grammar and fix globs". This replaces the orchestrator's provisional decision of the same day for mara-cp1skx (basic level, no pin). At the grammar level only comment nodes hold markers, so the test strings of `crates/mara-engine/src/code/tests.rs` need no exclusion. A machine runs `mara code add rust` once before the local checks.

- Maintainer, 2026-11-21 (mara-egn2sb): this repository's `[code]` names its Rust code with the one pattern `crates/**/*.rs`, tests included, chosen with the words "Glob everything" over keeping the two `src` patterns. The comment that mara-znydv4 put above the patterns (the test directories hold fixture files with real markers) was the orchestrator's unchecked sentence and is wrong: at the grammar level the test sources give no marker. Cost: a direct `project validate` parses 219 files instead of 110. Undo: name the two `src` directories again.
