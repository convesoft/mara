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
- The counts of one request are one `WorkCounts` value (`mara-engine`, module `work`). The host creates it and passes it by `&mut` to each engine call that does counted work. The code that does a piece of work counts it.
- A file read is one read of a file's contents by the host. A stat or a failed read is none.
- A built-in check step is one item or edge that a built-in check visits.
- `--debug-stats` makes the stats line the last line on stderr, after any diagnostic: also when the request fails or its arguments are rejected. Stdout stays the operation's.
- The stats line is a JSON object of exactly `format_version` (1) and `file_reads`, `hashes`, `parses`, `snapshot_builds`, `code_scans`, `subprocess_spawns`, `check_steps` and `program_runs`. Key order carries no meaning.

Corpus generator:

- Deterministic and seeded; writes the current item format.
- Produces items across documents with realistic relation counts and body sizes, and code files carrying markers.
- Chains of `Decomposes` and `Derives from` have a depth and fan-out that do not grow with corpus size: a larger corpus has more clusters, not deeper ones.
- Sizes: 1× (about 200 items), 4× and 16× on every pull request; 64× nightly.
- Generates into disposable temporary directories, never into the checkout.
- A seed and a size give byte-identical files on every machine and toolchain. The generator draws from SplitMix64, written in the test, never from a library generator. Each cluster draws from its own stream, keyed by seed and cluster number. A MID is a fixed timestamp plus the item number, then seeded random bits.
- 1× is 8 clusters. A cluster is 25 items of every engineering type in three documents, and 12 Rust files carrying `@mara implements` markers. It links only to itself and to the previous cluster, so a smaller corpus's clusters reappear unchanged in every larger one.
- Chains are at most 2 edges deep, with at most 2 chain edges into one item.
- Accepted items obey the engineering template's acceptance and lifecycle rules, and no built-in check fails.
- The configuration is the example of [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27). The schema is the engineering vocabulary, written with only the keys that [DES-SCHEMA-MODEL](project.mara.md#01M7BF6HA0KG3R3AATWM1P2X1G) shows, so each Verification and Validation has one method and each Term one alias.
- The configuration declares no code language and no program until their syntax exists. The issue that defines it declares them in the generated configuration.
- Each corpus also holds a `.git` directory, a `.gitignore`, ignored build output, an ignored document, a document outside the include patterns and a README.

Tests:

- The counter and scaling tests are the `performance` integration test of the `mara` crate, with the generator. They run the real binary with an empty environment.
- A case is the arguments and the exit status of one invocation. The harness asserts the status before it accepts the counts or the timing. Each operation adds its case to both tests.
- Two cases without an operation pin today's behaviour and stay: `mara --debug-stats` exits 0, and `mara --debug-stats --no-such-option` exits 2 after a diagnostic. Neither does work, and the first does not grow. Changing either behaviour changes its case.
- Engine code that runs inside operations, such as the document parser, adds no case: it counts its own work, and the cases of the operations that run it assert and time that work.
- Input tracking and the snapshot build also have cases below the CLI, in the same target: exact counts of a cold build and of later builds in the same process (nothing changed, one document edited, undeclared files changed) at every size, and the growth of a cold build's time.
- The tests check the stats line against their own list of counter names, never against `WorkCounts`.
- The scaling test runs on a release build only. One warm-up round, then five timed rounds that each run every size once. Growth per doubling between adjacent sizes s₁ < s₂ is (t₂ / t₁)^(1 / log₂(s₂ / s₁)) of their median times.
- The 64× run (`--ignored`) asserts counts only. The growth limit applies from 1× to 16×.

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
