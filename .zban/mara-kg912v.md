---
title: "Split crates/mara/src/inputs.rs: walk, code files, .gitignore"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
  - audit
blocked_by:
  - mara-h4z424
  - mara-dskbzq
  - mara-dv7ad1
discovered_from: mara-0mxw21
created_at: 2026-11-21T16:35:20Z
---

Goal: `crates/mara/src/inputs.rs` holds `Inputs`, the state and `Generation`; the rest lives in modules by concern.

Context: Found by the architecture audit after 0.4.0-alpha.4 (Gemini, at `2fa2c1b`, 2026-11-21); locations are as of that commit. Wave: split; pure restructuring, after the behaviour changes it is blocked by. The file is about 1,800 lines after mara-agdnwh, which put the code maps in fields of their own. Four workers suggested the same split: the walk (`Walk`, `Code`, `Directory`) into `inputs/walk.rs`; the code-file functions (`read_code`, `scan_code`, `code_file`, `scan_listed`, `scan`, `give_back`, `forget_code`, `same_scan`, `key_of`) into `inputs/code.rs`; the `.gitignore` helpers (`ignored`, `compile_gitignore`, `UNSUPPORTED_GITIGNORE`) into `inputs/gitignore.rs`.

- [ ] The three modules exist; no behaviour, result or work count changes
