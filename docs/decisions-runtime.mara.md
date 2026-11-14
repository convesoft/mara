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
