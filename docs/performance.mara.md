# Performance

## Requirements

---

<a id="01M7BF6H3A6YQ43351V9YSV3SQ"></a>

### Work follows what changed

| Type | Requirement |
| --- | --- |
| ID | REQ-WORK-FOLLOWS-CHANGE |
| Status | Accepted |
| Satisfies | [NEED-INTERACTIVE-SPEED](product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

A request performs derivation work only for inputs it reads; undeclared files and `.git` cause none. A host that keeps state between requests also limits that work to inputs that changed since the previous request: unchanged inputs, failed requests and idle time cause none.

---

<a id="01M7BF6H3K1M4HMG6CW2B85VQK"></a>

### Cost grows near-linearly with corpus size

| Type | Requirement |
| --- | --- |
| ID | REQ-NEAR-LINEAR-SCALING |
| Status | Accepted |
| Satisfies | [NEED-INTERACTIVE-SPEED](product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

On the generated corpus ([DES-PERFORMANCE-HARNESS](#01M7BF6H4HKH0B0TXQZM3VXASA)), each operation's time grows at most 2.3× per doubling from 1× to 16×. Built-in checks do work linear in items plus edges, and validation runs each program once.

---

<a id="01M7BF6H3T3589901T1AE0ADHM"></a>

### Reads do not wait for validation

| Type | Requirement |
| --- | --- |
| ID | REQ-READS-DURING-VALIDATION |
| Status | Accepted |
| Satisfies | [NEED-INTERACTIVE-SPEED](product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

A read issued during a long validation of the same project completes within its warm budget ([REQ-TIME-BUDGETS](#01M7BF6H47MJ8H16R3B4S7750T)).

---

<a id="01M7BF6H47MJ8H16R3B4S7750T"></a>

### Time budgets at 16×

| Type | Requirement |
| --- | --- |
| ID | REQ-TIME-BUDGETS |
| Status | Draft |
| Satisfies | [NEED-INTERACTIVE-SPEED](product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

On the reference machine at 16× corpus size, a cold CLI read takes at most 200 ms, a warm daemon read at most 20 ms and validation at most 1 s. These are starting targets, to adjust after the first measurements.

---

## Design

---

<a id="01M7BF6H4HKH0B0TXQZM3VXASA"></a>

### Work counters and corpus generator

| Type | Design |
| --- | --- |
| ID | DES-PERFORMANCE-HARNESS |
| Status | Accepted |
| Satisfies | [REQ-WORK-FOLLOWS-CHANGE](#01M7BF6H3A6YQ43351V9YSV3SQ) |
| Satisfies | [REQ-NEAR-LINEAR-SCALING](#01M7BF6H3K1M4HMG6CW2B85VQK) |

Counters:

- Engine and hosts count file reads, hashes, parses, snapshot builds, code scans, subprocess spawns, built-in check steps and program runs.
- A debug stats option exposes the counts, so tests check the real CLI and daemon.

Corpus generator:

- Deterministic and seeded; writes the current item format.
- Produces items across documents with realistic relation counts and body sizes, and code files carrying markers.
- Chains of `Decomposes` and `Derives from` have a depth and fan-out that do not grow with corpus size: a larger corpus has more clusters, not deeper ones.
- Sizes: 1× (about 200 items), 4× and 16× on every pull request; 64× nightly.
- Generates into disposable temporary directories, never into the checkout.

---

## Evidence

---

<a id="01M7BF6H4V7TRV0EZTK37530GE"></a>

### Performance audit of the previous implementation

| Type | Evidence |
| --- | --- |
| ID | EVD-PREVIOUS-IMPLEMENTATION-AUDIT |
| Status | Accepted |
| Supports | [GOAL-FAST-AT-SCALE](product.mara.md#01M7BF6H5JHS6GZ8CMB98MBTMM) |

Measured 2026-11-12 on commit `d7f0e73` (0.4.0-alpha.1): 202 items in 27 documents, 130 Rust files.

| Finding | Measurement |
| --- | --- |
| `rust-analyzer scip` ran on every read and twice per write; nothing was persisted | 14–20 s and about 3.3 GB per run; `item list` 16.9 s, against 0.055 s with code indexing off |
| One hash over every non-ignored file, `.git` included, guarded the whole cache | Dropped by any unrelated file, commit, failed request, write, or 5 idle minutes |
| Algorithms quadratic in item count | `project validate` 0.41 s at 1× and 75 s at 16× (×3.9 per doubling); `get` 3 s at 16× |
| One rule engine per item × rule; node IRIs formatted and parsed in inner loops; query definitions recompiled on every execution | 85–95% of validation CPU in one per-outcome helper; about 46% in IRI parsing |
| One mutex per project | A 0.115 s read took 4.1 s during a validation |

The report, flamegraphs and tools (scale-corpus generator, stub indexer, bench script) are on the maintainer's machine in `~/Work/local/workspace/projects/mara-performance-audit/`.

---

## Profiling

- Profile a release build with line tables: `[profile.profiling]` inheriting `release`, with `debug = "line-tables-only"`.
- `samply` gives CPU profiles, `perf` with `inferno` flamegraphs, `valgrind --tool=callgrind` exact instruction counts, `heaptrack` allocations.
- Unprivileged profiling needs `kernel.perf_event_paranoid` ≤ 1. `samply` on many-core machines needs `kernel.perf_event_mlock_kb` ≥ 2048.
