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

<a id="01M7BF6GSZ16MZQAGBY5M9TDKZ"></a>

### Write project logic in Soufflé Datalog

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-DATALOG |
| Status | Accepted |
| Justifies | [DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE) |

- Chosen: Soufflé. A program declares its typed input and output relations, mistakes are rejected before evaluation, and modules are native (`.include`, `.once`, `.comp`).
- Fallback if Soufflé becomes unworkable: Nemo. Not chosen first because it is dynamically typed and silently skips mismatching rows.
- Rejected: GraphQL. Computed fields need custom resolvers or Mara-specific directives.
- Rejected: SPARQL. It has no modules or typed input and output declarations, so it needed a JSON wrapper plus separate SHACL constraints.
- Rejected: Rego (`regorus`). It forbids recursive rules.

---

<a id="01M7BF6GT9PVXSKV1ANGEWA7WR"></a>

### Run Soufflé through its command line

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-AS-PROCESS |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |

- Chosen: Soufflé's command-line contract: a bundled child process natively, the Emscripten build in the browser worker.
- Rejected: the C++ interface ([`souffle/SouffleInterface.h`](https://souffle-lang.github.io/interface)) and [SWIG](https://souffle-lang.github.io/swig). They run only programs translated with `souffle -g` and compiled; the interpreter's `ProgInterface` leaves `run()` and `loadAll()` empty (`src/interpreter/ProgInterface.h:258`). Every rule change would need a C++ compiler on the user's machine, and none of it works in a browser.
- Rejected: in-process evaluation. Even with `SOUFFLE_ALLOW_SIGNALS` (`src/include/souffle/SignalHandler.h:96`), integer division by zero raises SIGFPE, and any C++ exception reaches `SignalHandler::error()`, which calls `exit(1)` (`SignalHandler.h:148–154`). A child process contains both.

---

<a id="01M7CMW6ZGC0PY8DVYNQDSBPRH"></a>

### Refuse program constructs that reach outside the evaluation

| Type | Decision |
| --- | --- |
| ID | ADR-PROGRAM-PROFILE |
| Status | Accepted |
| Justifies | [DES-PROGRAM-PROFILE](rules.mara.md#01M7CMW7051QVPFDWQBMB1A6K6) |
| Justifies | [REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0) |

- Chosen: a scan before every run refuses the constructs that read, write or execute outside the evaluation ([EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)), and a time limit bounds each evaluation.
- Rejected: trusting programs like build scripts. Validating an untrusted checkout would run its code.
- Rejected: sandboxing the child process with Landlock, seccomp or `sandbox-exec`. It is platform-specific and fails on kernels without Landlock.
- Rejected: forbidding records, ADTs and `autoinc()` to bound evaluation. Plain recursion with arithmetic or `cat` never ends either.

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
