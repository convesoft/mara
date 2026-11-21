---
title: "Catalog tests: what the review of mara-yk8drm left"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-yk8drm
created_at: 2026-11-21T17:48:36Z
---

Goal: the tests of the shipped grammar catalog fail when an entry is dropped, when a URL names another tag, or when a shipped extension or comment prefix is wrong.

Context: P3 findings of the review of mara-yk8drm (Codex, at `560549d`, 2026-11-21; log `~/.tmp/codex/review-mara-yk8drm-560549d/findings.md`). None is reached by a user today: the catalog is right, and the reviewer checked the shipped defaults by hand in a fresh project. Items: DES-GRAMMAR-ASSETS (docs/code/index.mara.md), VER-CODE-ADD (docs/code/verification.mara.md).

- [ ] `the_built_in_catalog_pins_the_fixture_grammars` and the ignored real-repository test (`crates/mara/tests/code/add.rs`) name rust and python themselves, so dropping an entry fails them; both read the parsed catalog, not the TOML's text
- [ ] The URL check compares the whole expected URL, not that `/v<version>/` occurs somewhere in it
- [ ] A test adds each catalog language to a project that declares no language and checks the extensions and comment prefixes `code add` writes (the real-repository test's project declares both languages already, `crates/mara/tests/code/main.rs`)
- [ ] VER-CODE-ADD's setup paragraph says which cases use the debug catalog override: the loopback cases do, the fixture check reads the embedded catalog, and the real-repository case uses the shipped one
