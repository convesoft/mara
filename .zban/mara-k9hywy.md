---
title: "Warm-row tests: what the re-check of mara-p27z9t left"
status: todo
type: task
priority: deferred
tags:
  - deferred
  - backlog
discovered_from: mara-p27z9t
created_at: 2026-11-21T14:53:16Z
---

Goal: the warm-row tests leave none of the gaps below.

Context: left by the re-check of mara-p27z9t (at `4749f8d`, integrated as the commit that closes it), each rated P2 or P3 there; the orchestrator moved them to the backlog, because none is a defect a user reaches and the timing cases need the watcher's derivation to come 2 s or more late. Items: VER-WORK-COUNTERS (docs/verification.mara.md), DES-DAEMON-SESSIONS (docs/daemon/index.mara.md), ADR-DEBUG-PAUSE-POINT (docs/authoring/writes.mara.md).

- `crates/mara/tests/daemon/warm.rs`: the move test asserts the derivation's reads and parses only, the schema test only reads, so an extra hash passes; the schema test expects two reads also when the first derivation came 2 s or more late and already settled the schema, and then fails on a correct one-read response.
- `crates/mara/tests/performance/warm.rs`, the delayed post-write test: when the handler is delayed after the hand-over, a watcher's read that the row allows can be charged to the write's response, which the test asserts to hold no read.
- `crates/mara/tests/performance/warm_scaling.rs`: watcher work done after a timed response is in the CPU sample but in no asserted count; the next untimed request drains it unchecked.
- `crates/mara/tests/support/derived.rs`: a report is written after its work is added to the session, so a response can hold work whose report is not yet there (the fixed assertion then fails where a repeat was meant).
- The second-project case of the no-invalidation test probes only `item list`: add the `query execute` probe the same-project cases have.
- ADR-DEBUG-PAUSE-POINT names one of the three situations in which a run is repeated (also: a setup request that no longer leaves the input unsettled; a move whose two files fall on both sides of the settle deadline).
- DES-DAEMON-SESSIONS says a scan that fell back is not kept; a fallback kept by a full scan is reused by `get` of a code location (same contents and grammar, so the same result): say which is meant.

- [ ] Each bullet closed or dropped with a reason
