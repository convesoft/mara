# Decisions

---

<a id="01M7BF6GTYJB4MCBXQSHWFXMYZ"></a>

### Write items as ordinary Markdown

| Type | Decision |
| --- | --- |
| ID | ADR-MARKDOWN-NATIVE-ITEMS |
| Status | Accepted |
| Justifies | [DES-ITEM-STRUCTURE](format.mara.md#01M7BF6GZDTNNW0Z0MX7FEG6ZF) |

- Chosen: an item is an HTML anchor, an ATX heading and a GFM table.
- Rejected: `:::mara` directive blocks. GitHub shows them as raw text, without highlighting, rendering or link navigation.

---

<a id="01M7BF6GVAEPTJR91QAR6QRW24"></a>

### Store the MID in an uppercase HTML anchor

| Type | Decision |
| --- | --- |
| ID | ADR-MID-IN-HTML-ANCHOR |
| Status | Accepted |
| Justifies | [DES-MID-ANCHOR](identity.mara.md#01M7BF6H1MB5RR3RS43B3CJHAR) |

- Chosen: `<a id="<MID>"></a>` is the only place a MID is stored; it is the identity and the link target at once.
- Rejected: heading slugs as link targets. They change with the title.
- Rejected: lowercase MIDs in anchors and links. GitHub already matches fragments case-insensitively ([EVD-GITHUB-FRAGMENT-MATCHING](identity.mara.md#01M7BF6H2MHAVK1Z5HCXR1V289)).

---

<a id="01M7BF6GVK3EZ2G1CYXSEB9XC3"></a>

### Author typed relations only in the source item's table

| Type | Decision |
| --- | --- |
| ID | ADR-RELATIONS-ONLY-IN-TABLE |
| Status | Accepted |
| Justifies | [DES-RELATION-LINKS](identity.mara.md#01M7BF6H29JGHG65SV91K6D11V) |
| Justifies | [REQ-EDIT-RELATIONS](authoring/index.mara.md#01M7BF6GN8DJ66NPBMQK83H9PW) |

- Chosen: one row on the source item, under the forward relation name, holding a Markdown link.
- Rejected: `[[ID]]` and `[[relation:target]]` syntax. It is not Markdown and does not navigate on GitHub.
- Rejected: rows under the inverse name on the target item. One edge would have two spellings.
- Rejected: an inverse name in `relation add`, written as the forward row on the other item. The operation would edit an item it was not pointed at.

---

<a id="01M7CMW6Y0WFD4W6951906MXQY"></a>

### Keep configuration and schema as TOML files under `.mara/`

| Type | Decision |
| --- | --- |
| ID | ADR-TOML-PROJECT-FILES |
| Status | Accepted |
| Justifies | [DES-PROJECT-CONFIGURATION](project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27) |

- Chosen: `.mara/config.toml` and `.mara/schema.toml`, each with its own format version starting at 1.
- Rejected: the 0.3.0 names, `.mara/project.toml` and `.mara/schema.yaml`. YAML needs a second parser, and the format versions would continue at 4.
- Rejected: one `mara.toml` at the project root. A schema edit would change the configuration hash, and `project init` could not write the schema as its own file.

---

<a id="01M7BF6GWEX6QCDC1YGR54P23Z"></a>

### Distribute prebuilt binaries as npm packages

| Type | Decision |
| --- | --- |
| ID | ADR-SCRIPT-FREE-NPM-PACKAGES |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: a dispatcher package with one optional dependency per platform.
- Rejected: an install-time downloader. It depends on lifecycle scripts, which restricted environments disable.

---

<a id="01M7CMW6YNXSQAS7H0ATKG4GH0"></a>

### Keep the 0.3.0 platforms and Node version

| Type | Decision |
| --- | --- |
| ID | ADR-KEEP-PLATFORMS-AND-NODE |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: Linux x64 and arm64 (glibc), macOS arm64 and Node 24, as in 0.3.0.
- Rejected until requested: Windows. It needs a Soufflé MSVC build, a patch for explanations (the explain console throws `No explain/explore provenance on Windows`) and named-pipe IPC for the daemon.
- Rejected until requested: Intel macOS and musl Linux.
- Rejected: Node 22. It reaches end of life in April 2027.

---

<a id="01M7CQ57SQY6M97NEDTA27K5WE"></a>

### Embed Mara in Node through a native addon

| Type | Decision |
| --- | --- |
| ID | ADR-NODE-NATIVE-ADDON |
| Status | Accepted |
| Justifies | [DES-PACKAGES](distribution.mara.md#01M7BF6GWZ72PGJNA3VQNZMDC8) |

- Chosen: an N-API addon over the same Rust host code as the CLI, shipped in the native packages.
- Rejected: the WASM engine with a host written in JavaScript. A second host would have to match the Rust one in file access, hashing, caching and running Soufflé.

---

<a id="01M7BGRHX3NVNZS0NGQ0BKPSH1"></a>

### Deliver agent guidance from the binary, not a skill

| Type | Decision |
| --- | --- |
| ID | ADR-GUIDE-IN-BINARY |
| Status | Accepted |
| Justifies | [REQ-AGENT-GUIDE](agents.mara.md#01M7BGRHWB2MGXEY5G41YP8E0G) |

- Chosen: the guide is compiled into the binary and delivered by `mara prime` and the MCP server instructions, as [zban](https://github.com/convesoft/zban) does.
- Rejected: a standalone skill, as in 0.3.0. It was installed separately and had to be matched to the binary's version by hand.

---

<a id="01M7BH0XEYXZXY3ZD8NNYGS0GM"></a>

### Without a project, advertise only initialization

| Type | Decision |
| --- | --- |
| ID | ADR-INIT-NOTE-WITHOUT-PROJECT |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: outside a project, `mara prime` and the MCP instructions say how to initialize one, and nothing more.
- Rejected: the full guide everywhere. It fills the context of projects that do not rely on Mara.
- Rejected: no output at all. Nothing would tell a user or agent how to start.

---

<a id="01M7CMW6YBVK2FYK2AQA7XAT6Y"></a>

### Keep the agent guide independent of the project's schema

| Type | Decision |
| --- | --- |
| ID | ADR-STATIC-AGENT-GUIDE |
| Status | Accepted |
| Justifies | [DES-AGENT-GUIDE](agents.mara.md#01M7BGRHWQHQJJHC6XCXVDWDM8) |

- Chosen: one guide per binary, which sends the agent to schema inspection.
- Rejected: listing the project's types and ID prefixes. MCP instructions are sent once per connection, so the list goes stale after a schema edit, and a large schema exceeds the size limit.

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
- Decided by the orchestrator on the maintainer's behalf on 2026-11-16: the daemon also has scaling cases, since most requests take that path. Chosen for them: the daemon's process CPU clock, in nanoseconds and with its exited threads. Rejected: `utime` and `stime` of `/proc/<pid>/stat`, in 10 ms ticks, coarser than a warm request.
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
