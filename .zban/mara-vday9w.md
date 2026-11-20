---
title: Concurrent evaluations of one process share one interrupt flag
status: todo
type: bug
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-xqcnqw
created_at: 2026-11-20T13:42:55Z
---

Goal: SIGINT, SIGTERM or SIGHUP during an evaluation removes every running evaluation's private directory before Mara ends, also when one process runs several at once.

Context: `rules::interrupt::Deferred` (`crates/mara/src/rules.rs`) keeps one process-wide `idle` flag. `mara mcp --no-daemon` serves tool calls on blocking threads, so two evaluations can run at once: when the first ends it sets `idle`, and a signal that arrives while the second still runs takes its default action at once, leaving the second's directory in the temporary directory. The daemon is not affected: it blocks those signals and ends evaluations through `rules::cancel`.

- [ ] Count the running evaluations instead of a flag, so the default action waits for the last
- [ ] Test with two slow queries through `mara mcp --no-daemon` and SIGTERM
