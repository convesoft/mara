# Runtime decisions

---

<a id="01M7BF6GRR25T2FSC1KYQYW41V"></a>

### Key derived values by content

| Type | Decision |
| --- | --- |
| ID | ADR-CONTENT-KEYED-VALUES |
| Status | Accepted |
| Justifies | [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) |

- Chosen: each derived value has its own key, built from the hashes of the inputs it reads.
- Rejected: one fingerprint over the repository. Any unrelated file, `.git` change, failed request or idle period dropped everything ([EVD-PREVIOUS-IMPLEMENTATION-AUDIT](performance.mara.md#01M7BF6H4V7TRV0EZTK37530GE)).
- Rejected: keys from paths or timestamps.

---

<a id="01M7CMW6YZ0S1DPP7T8AD7A3DQ"></a>

### Filter inputs through the project's `.gitignore` files

| Type | Decision |
| --- | --- |
| ID | ADR-GITIGNORE-FILTERS-INPUTS |
| Status | Accepted |
| Justifies | [DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY) |

- Chosen: include patterns select documents and code files, and `.gitignore` files in the project tree exclude them.
- Rejected: configuration patterns only. Every project would list `target/`, `node_modules/` and its build output itself.
- Rejected: Git's global excludes and `.git/info/exclude`. They differ per machine, so hosts would disagree.

---

<a id="01M7BF6GS1109M3YV9WHTT4XTK"></a>

### Keep caches in memory

| Type | Decision |
| --- | --- |
| ID | ADR-MEMORY-ONLY-CACHES |
| Status | Accepted |
| Justifies | [DES-DAEMON-SESSIONS](daemon.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7) |

- Chosen: no on-disk store. Documents and code markers rebuild in milliseconds to seconds, so a restart is cheap.
- Rejected until a measured cost requires it: an on-disk store.

---

<a id="01M7BF6GS99YFG0VNDCJ1QQRCB"></a>

### Start the daemon on demand and watch by default

| Type | Decision |
| --- | --- |
| ID | ADR-ON-DEMAND-DAEMON |
| Status | Accepted |
| Justifies | [DES-DAEMON-LIFECYCLE](daemon.mara.md#01M7BF6GR80WA207P4BBFWW671) |
| Justifies | [DES-DAEMON-SESSIONS](daemon.mara.md#01M7BF6GRG3NJZTA29TYVRVFT7) |

- Chosen: clients start the daemon in the background; it watches declared inputs, so the next request is already warm, and exits when idle.
- Rejected: explicit start only, as in 0.4.0-alpha.1. The CLI never started a daemon, so most calls ran cold.

---

<a id="01M7CMW6Z9MMB1705N02S9CCYG"></a>

### Run one daemon per Mara version

| Type | Decision |
| --- | --- |
| ID | ADR-DAEMON-PER-VERSION |
| Status | Accepted |
| Justifies | [DES-DAEMON-LIFECYCLE](daemon.mara.md#01M7BF6GR80WA207P4BBFWW671) |

- Chosen: each version has its own lock and socket. Daemons of different versions coexist and exit when idle.
- Rejected: one daemon per OS user that exits when a client of another version arrives. Projects pinned to different versions would stop each other's daemon on every call and never run warm.

---

<a id="01M7CQ57T4TYP96W06VB9P2W96"></a>

### Enable browser access only by flags on `daemon start`

| Type | Decision |
| --- | --- |
| ID | ADR-HTTP-BY-FLAG |
| Status | Accepted |
| Justifies | [DES-BROWSER-LISTENER](daemon.mara.md#01M7CQ57TJY788DMPBN4BX1SEM) |

- Chosen: the user turns the listener on and names origins and projects when starting the daemon. Nothing is persisted.
- Rejected: allowed origins in the project configuration. A cloned repository could grant a web origin access to itself.

---

<a id="01M7BF6GSKJFB28V4HFVSED44G"></a>

### Link code through comment markers, not SCIP

| Type | Decision |
| --- | --- |
| ID | ADR-COMMENT-MARKERS-NOT-SCIP |
| Status | Accepted |
| Justifies | [DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1) |

- Chosen: `@mara` comment markers, located by file and line or by a tree-sitter declaration.
- Rejected: SCIP indexers. `rust-analyzer scip` cost 14–20 s and about 3.3 GB on every read ([EVD-PREVIOUS-IMPLEMENTATION-AUDIT](performance.mara.md#01M7BF6H4V7TRV0EZTK37530GE)).

---

<a id="01M7BF6GTKYKE4FRPM8A80VXFJ"></a>

### Gate pull requests on work counters and growth ratios

| Type | Decision |
| --- | --- |
| ID | ADR-COUNTERS-GATE-PULL-REQUESTS |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: pull requests are gated on exact work counters and growth ratios ([VER-WORK-COUNTERS](verification.mara.md#01M7BF6HGG6ET88HS641Y5676G), [VER-SCALING](verification.mara.md#01M7BF6HGRNQ5SJVNP89G1CCBW)); absolute time is checked only nightly ([VER-TIME-BUDGETS](verification.mara.md#01M7BF6HH06WEJ67P04H22BM66)).
- Rejected: absolute time thresholds on pull requests. Timings on shared machines varied by about ±20% in the audit; counts and ratios did not.

---

<a id="01M7DTPFQMCA1VN75741RKEDR6"></a>

### Time the scaling test in CPU time, with one early look and 60 runs near the limit

| Type | Decision |
| --- | --- |
| ID | ADR-SCALING-CPU-TIME |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: the median CPU time of each size. After five rounds, one look: a pair whose growth is more than four standard errors below the limit passes; every other pair gets 60 runs of both sizes and is judged once on the medians of all runs. The maintainer chose this stopping rule. Beside one looping 16-job release build (load 14–31), 20 of 20 runs passed in 31–49 s, with 20–26 of 32 pairs at 60 runs; at load 13–16 without it a run took about 24 s.
- Chosen margin: four standard errors, from the standard deviation of the pair's ten runs pooled over its two sizes, because a false pass can happen only at the look: in a simulation with 15–25% log-normal noise, 2.0× then 2.4× or 2.5× passed at most 0.012 points more often than with 60 fixed runs, and 3.5 errors still exceeded that.
- Rejected: repeated looks until a pair is clear, then the median of up to 60 runs. They select a passing estimate: at 2.0× then 2.4× with 15% noise they passed 2.32% against 0.66% for 60 fixed runs.
- Rejected: failing a pair still unresolved at 60 runs. It fails unchanged code whose growth lies within the noise of 60 runs below the limit.
- Rejected: two standard errors from the median absolute deviation of five runs per size. Five runs underestimate the noise: the look added up to 2.6 points of passes above the limit.
- Rejected: 60 runs of every pair. It took 35 s instead of 24 s at load 13–16, with no measurably fewer false passes.
- Rejected: wall-clock time. It counts waiting for a CPU: under two looping release builds, 11 of 20 runs failed at five rounds against 6 of 20 with CPU time.
- Rejected: the minimum over rounds. A short run escapes contention more often than a long one, so the minimum inflates growth: 7 to 10 of 10 busy runs failed, against 1 to 3 with the median.
- Rejected: pinning the run to one CPU, or disabling transparent huge pages or address randomization for it. None narrowed the spread.
- Not covered: CPU time excludes time a process waits on I/O or locks, so a blocking latency that grows faster than the corpus does not fail this test.
- Rejected by the maintainer: raising the limit to 2.4 so that the test also passes on a machine whose memory bandwidth other processes saturate. Correct code would pass there by only 0.04–0.09, and the weakest planted fault would fail by 0.15 instead of 0.25; the maintainer chose to rerun such a failure without that load instead.

---

<a id="01M7F8TYKM1HAKA1YQ5NFNKBJC"></a>

### Alternate the hub's other row keys between the lengths of `Type` and `ID`

| Type | Decision |
| --- | --- |
| ID | ADR-HUB-ROW-KEYS |
| Status | Accepted |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen: the hub's other rows alternate between `Tags` and `No`. A comparison of two keys of different lengths ends at the length: with only `Tags` rows, a per-edge scan for the ID cost about 0.4 ns per row and a scan for the type, whose key has the length of `Tags`, about 1.0 ns. The number of rows and the counts stay, and on correct code the hub's CPU time and growth did not change: 166 and 163 ms at 16× quiet, and growth per doubling of 1.95–2.26 under load, against 2.00–2.27 before.
- Measured with each scan planted in a copy, as growth from 4× to 16× before and after; quiet is load below 3 and 9 rounds, the rest the gate's 60 runs per size. The scan for the ID (`value("id")` per edge in `Checker::edge`): 2.46–2.57 and 2.86 quiet, 2.37 and 2.57 beside other projects' tests (load 9–29), 2.46 and 2.61–2.71 beside 32 spinning processes (load 19–48), 2.41 and 2.62–2.65 beside 24 spinning and 8 memory-writing processes (load 34–52). The scan for the type (in `target_type`): 2.98 and 2.81 quiet, 2.67 and 2.55 beside other projects' tests, 2.62 after beside the spinning processes, 2.70 and 2.56 beside the memory-writing ones. Under load the smaller margin of the two scans grew from 0.07–0.16 to 0.25–0.31.
- Rejected: leaving the shape. The scan for the ID exceeded the limit by only 0.06–0.22 under load.
- Rejected: four times as many `Tags` rows. The scan for the ID grew 2.92 at load 10, but the hub took 60% more CPU time per run (198 to 316 ms at 16×).
- Rejected: a block of `Tags` rows, then a block of as many `No` rows. Both scans grew 3.15 quiet, but the hub took 11% more CPU time per run, and correct code failed the gate in 2 of 2 runs beside the memory-writing processes (2.35, 2.36).
- Rejected: only `No` rows. The scan for the ID grew 3.09 quiet, but the scan for the type fell to 2.54.

---

<a id="01M7F9N0JK7BSWE98V9VTCGBV0"></a>

### Exempt the converter from the scaling test

| Type | Decision |
| --- | --- |
| ID | ADR-NO-CONVERTER-SCALING |
| Status | Accepted |
| Justifies | [REQ-NEAR-LINEAR-SCALING](performance.mara.md#01M7BF6H3K1M4HMG6CW2B85VQK) |
| Justifies | [DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) |

- Chosen by the maintainer on 2026-11-15: `project convert` has no scaling case, and its counter case runs on a small fixed 0.3.0 input. Each user runs the conversion once.
- Rejected: a 0.3.0 mode of the corpus generator. It is test tooling for one operation.
- Rejected: copies of the `v0.3.0` corpus at 1×, 4× and 16× with their MIDs and IDs made unique. It is test tooling for one operation too.
