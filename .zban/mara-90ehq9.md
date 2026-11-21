---
title: The grammar catalog, the pin's read-back and the configuration check comment prefixes and rules by one rule
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
related:
  - mara-d41wg3
  - mara-54em6a
discovered_from: mara-779tms
created_at: 2026-11-21T21:32:43Z
---

Goal: what `mara code add` accepts from its catalog and what it checks after writing the pin are the checks the configuration itself applies, and the tests read the catalog with the real parser.

Context: found by the architecture audit after 0.4.0-alpha.5 (Gemini, at `368fad1`, 2026-11-21); wave: seam. Locations are as of that commit. Exotic today: only the shipped catalog and the debug-only override reach the first point, and the second needs a write that damages `[rules]`.
- `catalog::check` (`crates/mara/src/cli/code/catalog.rs`, near 45–106) checks names, versions and URLs through `mara_engine::config`, but for `comment_prefixes` only that the list is not empty; `config::prefixes` (`crates/mara-engine/src/config.rs`, near 350–380) also refuses an empty prefix, white space and duplicates. A catalog entry with such a prefix passes the catalog and fails later in `pin::read_back` as `unreadable-edit`.
- `pin::read_back` (`crates/mara/src/cli/code/pin.rs`, near 135–172) compares `[rules]` before and after the edit only by whether it exists, not by its contents (REQ-REFUSED-EDIT-CHANGES-NOTHING, the read-back guard of DES-WRITE-PROCEDURE).
- `catalog.rs` is private to the binary crate, so `crates/mara/tests/code/add.rs` (near 508–555) parses `catalog.toml` by hand, twice, differently (the cause of mara-d41wg3).

- [ ] One exported check of comment prefixes serves the configuration and the catalog
- [ ] `pin::read_back` compares the rules table's contents, line numbers aside
- [ ] The catalog parser is reachable from the tests, and `add.rs` uses it (closes mara-d41wg3's first item)
