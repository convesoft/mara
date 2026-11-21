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
