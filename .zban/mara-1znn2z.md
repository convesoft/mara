---
title: Read program outputs without holding a JSON tree of the whole file
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-bx82k0
created_at: 2026-11-16T17:12:29Z
---

`mara_engine::evaluation::read_output` parses an output file into a `serde_json::Value` before checking its rows. A file may reach the output-size limit of 64 MiB (DES-RULE-EXECUTION), and the tree of such a file takes several times that in Mara's own process, once per consumed relation. Only a program that writes near the limit reaches it; the outcome stays unavailable or correct either way.

- [ ] Deserialize the rows directly into `Vec<Vec<Value>>` (or stream them), keeping every check of `read_output`
