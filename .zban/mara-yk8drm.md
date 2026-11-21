---
title: Decide what the shipped grammar catalog contains
status: todo
type: task
tags:
  - maintainer
created_at: 2026-11-21T13:47:07Z
---

Goal: `mara code add <language>` works without `--repository` for the languages the maintainer chooses, because the built-in catalog names where each grammar is fetched from and its checksum.

Context: mara-h0hxa3 (integrated as `10f36ad`) built `mara code add` and the catalog mechanism and tested them with a catalog the tests supply. The shipped catalog, `crates/mara/src/cli/code/catalog.toml`, is empty: nothing is published for it, and what it names is the maintainer's to decide. Until then `mara code add <language>` without `--repository` is refused with `unknown-grammar`, naming the option to use. docs/code/index.mara.md has the matter under `## Open questions`; DES-GRAMMAR-ASSETS says what an entry holds (a version, the URL of the `.wasm`, the URL of its `queries/tags.scm`, the checksum, the file extensions and the comment prefixes); ADR-GRAMMAR-TRUST-ON-FIRST-USE says catalog entries carry their checksums.

Recommended by the worker and the orchestrator: rust 0.24.2 and python 0.25.0, the releases the test fixtures already use (the checksum of rust 0.24.2 is the one in the docs' example), with the `.wasm` from the tree-sitter release assets and `tags.scm` from the same tag of the repository, once the maintainer confirms the URLs and that the `tags.scm` at the tag equals the fixture's copy. An ignored test in `crates/mara/tests/code/add.rs` adds each catalog language from its real repository; run it once by hand when the entries are in.

- [ ] The maintainer names the languages, versions and URLs
- [ ] The entries are added with their checksums, the ignored test passes once against the real repositories, and the bullet under `## Open questions` in docs/code/index.mara.md is removed
