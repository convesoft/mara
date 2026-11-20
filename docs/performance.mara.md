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

On the generated corpus ([DES-PERFORMANCE-HARNESS](#01M7BF6H4HKH0B0TXQZM3VXASA)), each operation's time except that of `project convert` grows at most 2.3× per doubling from 1× to 16× ([ADR-NO-CONVERTER-SCALING](decisions.mara.md#01M7F9N0JK7BSWE98V9VTCGBV0)). Built-in checks do work linear in items plus edges, and validation runs each program once.

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
- A request that the daemon serves reports the daemon's counts for its project's session: the work done since the session's previous response, the opening of the session and background re-derivation after a watched change included. The client adds none, and starting a daemon is not counted.
- `--debug-stats` makes the stats line the last line on stderr, after any diagnostic: also when the request fails or its arguments are rejected. Stdout stays the operation's.
- The stats line is a JSON object of exactly `format_version` (1) and `file_reads`, `hashes`, `parses`, `snapshot_builds`, `code_scans`, `subprocess_spawns`, `check_steps` and `program_runs`. Key order carries no meaning.
- The Node client's and the browser workspace's debug stats option reports, beside each call's result and never inside it, the object that the call's stats line would hold.

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
- It also declares Rust at the basic level in `src/**/*.rs` and `tests/**/*.rs` with the prefix `//` ([DES-CODE-MARKERS](code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)), so the corpus's markers are code links; none names the Verification that the delete case removes. It declares the `engineering` template's programs and entries ([DES-PROJECT-INIT](project.mara.md#01M7FVWHVXW71CWC0HJ52NTEQJ)).
- `query gate` names one accepted requirement of the last cluster and supplies a candidate and a complete `verification_result` file with one `pass` row, at the items' current revisions, per `Verifies` edge, `results.json` in the corpus root. `query report` names the retired requirement of the last cluster and explains its failing row of `implementation-ready`, `not_accepted`.
- Each corpus also holds a `.git` directory, a `.gitignore`, ignored build output, an ignored document, a document outside the include patterns and a README.

Tests:

- The counter and scaling tests are the `performance` integration test of the `mara` crate, with the generator. They run the real binary with an empty environment, but for `MARA_NO_DAEMON=1` in direct-mode cases and, in daemon cases, the test's own runtime directory as `XDG_RUNTIME_DIR` on Linux and as `TMPDIR` on macOS.
- A case is the arguments and the exit status of one invocation. The harness asserts the status before it accepts the counts or the timing. Each operation adds its case to both tests.
- A case of `mara mcp` is one session of the real server: the `initialize` handshake, `tools/list` and one tool call, whose stats line is that call's. A session without a call does no work.
- `project convert` has no scaling case. Its counter case runs the real CLI on a small fixed 0.3.0 input, a hand-written fixture of the test and not generator output ([ADR-NO-CONVERTER-SCALING](decisions.mara.md#01M7F9N0JK7BSWE98V9VTCGBV0)).
- A case of the release-build test starts a daemon with `MARA_TEST_IDLE_SECONDS=1` and opens a session: 3 s later `mara daemon status` still lists that session. The test then stops the daemon and finds no `MARA_TEST_IDLE_SECONDS` in the binary's bytes ([DES-IDLE-SESSIONS](daemon/index.mara.md#01M7CQ57TBWFN78CCMEAY6XFTF)).
- Two cases without an operation pin today's behaviour and stay: `mara --debug-stats` exits 0, and `mara --debug-stats --no-such-option` exits 2 after a diagnostic. Neither does work, and the first does not grow. Changing either behaviour changes its case.
- Engine code that runs inside operations, such as the document parser, adds no case: it counts its own work, and the cases of the operations that run it assert and time that work.
- Input tracking and the snapshot build also have cases below the CLI, in the same target: exact counts of a cold build and of later builds in the same process (nothing changed, one document edited, undeclared files changed) at every size, and the growth of a cold build's time.
- The tests check the stats line against their own list of counter names, never against `WorkCounts`.
- The scaling test runs on a release build only. One warm-up round, then five timed rounds that each run every size once. Growth per doubling between adjacent sizes s₁ < s₂ is (t₂ / t₁)^(1 / log₂(s₂ / s₁)) of their median times.
- A run's time is CPU time, user plus system, of the test thread and of the `mara` process it waits for, read with `clock_gettime` and `wait4` on Linux and macOS. Time spent waiting for a CPU does not count.
- A daemon case adds the daemon's CPU time over the run, read from its process CPU clock (`clock_getcpuclockid` and `clock_gettime` on Linux, user plus system time of `proc_pid_rusage` on macOS), which counts its exited threads but not its child processes. The run starts with a reading before the change, taken while the daemon is idle, and ends with one taken once its CPU time has stopped advancing for 100 ms longer than the watcher's debounce after the response. So re-derivation counts in the run of the request that needed it, whether the watcher or the request did it. Before each run, outside the timed span, the test restores the edited document, sends a request and waits for the daemon to be idle.
- The daemon cases are `search`, `item list` and `project validate`, each repeated on settled inputs, and `search` and `item list` after one document edit. They run no program, whose `souffle` processes the daemon's clock would miss; the direct-mode cases time program runs.
- After the five rounds, each pair of adjacent sizes is looked at once. A pair whose growth is more than four standard errors below the limit passes there. Every other pair gets 60 timed runs of each of its two sizes, interleaved and with no look in between, and is judged once: it passes when the growth of the medians of all its runs is at most the limit, and fails otherwise.
- At that look, the standard error of the growth's logarithm is √(π/2) × s × √(2 / 5) / log₂(s₂ / s₁), where s is the standard deviation of the ten runs' logarithms around their size's mean, pooled over the two sizes.
- The scaling test holds on a machine shared with other builds and tests, or with a CPU-bound process on every hardware thread. Unchanged code passed beside a looping 16-job release build (load 14–31), other projects' tests (load 9–20) and 32 spinning processes on 32 hardware threads (load 50–58).
- It is not defined for a machine whose memory bandwidth other processes saturate, where unchanged code exceeded the limit beside 8 processes writing 64 MiB buffers. A failure there is rerun without that load.
- The 64× run (`--ignored`) asserts counts only. The growth limit applies from 1× to 16×.
- A case that writes starts each run from the same files: before the run, outside the timed span, the test restores what the previous run wrote.
- Both tests also run `project validate`, `relation list`, `item rename` and `item move` of the hub item, and `project fix` after the hub item is moved to another directory by hand, on a hub project: 1000 × size items that link to one item whose `Type` and `ID` rows follow 1000 × size other rows, so late metadata meets many incoming edges. The other rows' keys alternate between `Tags` and `No`, as long as `Type` and `ID`, so a scan of the hub's rows for either compares every other key in full. The hub project also holds the Rust files of [VER-CODE-WORK-COUNTERS](code/verification.mara.md#01M7JKT47QNN11F4ZG7SG03CG8).
- Check steps do not count work per row, per holder of a shared key or per finding; scaling cases gate it, each on a project of one shape: an item with 1000 × size repeated `ID`, `Type` or `Status` rows, other values of a single-valued enum field, relation rows to one target or to as many targets, or undeclared keys; 500 × size items that share one ID or one MID; 1000 × size broken links on one line.

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
