---
title: "Program parser: quadratic union resolution and repeated inheritance colons"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-wrbjte
created_at: 2026-11-16T14:53:44Z
---

Goal: two low-priority differences of the program parser (crates/mara-engine/src/program/), found by the review of mara-wrbjte.

- [ ] Large forward-declared unions resolve quadratically: the type traversal in `resolve.rs` (`Resolver::kinds`) restarts at a union's first member after resolving each dependency. Resume at the next member instead, and test with a union of many forward-declared members
- [ ] Soufflé accepts repeated inheritance colons (`.comp C : A : B { }`), which Mara refuses. Check what Soufflé makes of them, then admit them with the same meaning or record the refusal in DES-PROGRAM-PROFILE (docs/rules-facts.mara.md)
