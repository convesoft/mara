---
title: Contain grammar modules that break tree-sitter's store assertions
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-s9gak4
created_at: 2026-11-20T14:26:15Z
---

Tree-sitter's WebAssembly store (`wasm_store.c`) asserts on several conditions of a grammar module, and an assertion ends the process with SIGABRT and no result. mara-s9gak4 checks the module's exports before loading it (`crates/mara/src/grammar/module.rs`): the language function and the initializers have the signatures the store calls. A module that passes and still breaks another assertion, such as a language object whose lexer or scanner function indices are outside the module's function table, or whose scanner functions have other signatures (called unchecked), still ends the process. Only a pin whose checksum names such a file reaches it; DES-GRAMMAR-ASSETS states the limitation.

- [ ] Decide how far to contain it: validate the language object's tables before use, load and parse in a child process, or patch the runtime upstream
- [ ] A test with such a module that ends with `unavailable-grammar` and no crash record
