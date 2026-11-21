---
title: "`--debug-stats` is read from the raw arguments, not the parsed flag"
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-v9tfhw
created_at: 2026-11-21T09:40:12Z
---

Goal: `--debug-stats` means the flag only where the CLI's parser reads it as the flag, as mara-v9tfhw made `--no-daemon` do.

Context: `crates/mara/src/main.rs:13` decides whether to print the stats line with `arguments.iter().any(|argument| argument == "--debug-stats")`. So `mara item list --id --debug-stats` prints a stats line, although `--debug-stats` is the value of `--id`. `requested_json` in `crates/mara/src/cli.rs` scans the raw arguments for `--format` the same way, so that rejected arguments are reported in JSON. DES-PERFORMANCE-HARNESS (docs/performance.mara.md) wants the stats line after rejected arguments too, so a fix must decide what counts as the flag when parsing fails part way. Items: DES-OPERATIONS (docs/hosts/index.mara.md), DES-PERFORMANCE-HARNESS.

- [ ] Decide which arguments count as `--debug-stats` and `--format` when parsing fails, and record it in DES-OPERATIONS
- [ ] Take both from the parser where it succeeds; a test with each as an option's value
