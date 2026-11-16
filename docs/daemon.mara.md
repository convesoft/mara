# Daemon

## Requirements

---

<a id="01M7BF6GQK5GM340Z7CP4C83B8"></a>

### The first call never waits for the daemon

| Type | Requirement |
| --- | --- |
| ID | REQ-FIRST-CALL-IN-PROCESS |
| Status | Accepted |
| Satisfies | [NEED-INTERACTIVE-SPEED](product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

A CLI, MCP or Node client that finds no daemon starts one in the background and runs its current request in-process. Later calls use the daemon.

---

<a id="01M7BF6GQSA67MYWBHWA6VYA3E"></a>

### Stale state is never returned as current

| Type | Requirement |
| --- | --- |
| ID | REQ-NEVER-STALE |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

Every request checks the declared inputs on disk. When one has changed, the response waits for the rebuild.

---

<a id="01M7BF6GR0N9J4C8Z5QQ4CSCB7"></a>

### Browsers reach the daemon only when authorized

| Type | Requirement |
| --- | --- |
| ID | REQ-BROWSER-ACCESS-AUTHORIZED |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

The daemon serves browsers over loopback HTTP only, and only requests that carry the session token, come from an allowed origin and target an allowed project.

---

## Design

---

<a id="01M7BF6GR80WA207P4BBFWW671"></a>

### Lifecycle

| Type | Design |
| --- | --- |
| ID | DES-DAEMON-LIFECYCLE |
| Status | Accepted |
| Satisfies | [REQ-FIRST-CALL-IN-PROCESS](#01M7BF6GQK5GM340Z7CP4C83B8) |

- Each Mara build has its own lock and socket in the runtime directory (`mara/`, mode 0700, in `XDG_RUNTIME_DIR`, or `/run/user/<uid>` when it is unset, on Linux; in `TMPDIR` on macOS). A build is its version with the path, size and modification time of its executable ([ADR-DAEMON-PER-VERSION](decisions-runtime.mara.md#01M7CMW6Z9MMB1705N02S9CCYG)). The lock ensures one instance per build when several clients start it at once; the socket path stays under the 108-byte Unix limit.
- When that base names no directory, or the socket path would pass the limit, every call runs in direct mode.
- A client connects only to the daemon of its own build. Daemons of different builds run side by side. A Node client's build is that of the `mara` binary of its native package, which it starts as its daemon.
- Clients send every document operation to the daemon except `project init` and `project convert`, which run in the calling process, as `prime` does.
- A client that finds no daemon of its build starts `mara daemon start`, with its project as `--project`, detached from its standard streams, and serves its request in-process. A daemon opens the session of the project that `--project` names or that holds its working directory, if any, as soon as it starts.
- A client falls back to direct mode only before it dispatches a request. A dispatched request is never replayed on another backend: when the daemon ends before answering, the request fails with `daemon-lost` ([DES-OPERATIONS](hosts.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS)).
- The daemon exits when its last session is dropped ([DES-IDLE-SESSIONS](#01M7CQ57TBWFN78CCMEAY6XFTF)), or after the idle period when it opened none, unless it listens for browsers ([DES-BROWSER-LISTENER](#01M7CQ57TJY788DMPBN4BX1SEM)).
- A client runs in direct mode, neither connecting to a daemon nor starting one, when `CI` is set, `MARA_NO_DAEMON=1` is set or the direct-mode flag is given.
- `mara daemon status | start | stop` give explicit control. `status` prints whether the daemon of the client's build runs, its process ID and the root of each open session. `start` and `stop` exit 0 also when there is nothing to do; `stop` returns once the daemon has exited.
- `mara daemon stop` and SIGTERM let the writes the daemon runs finish, end its other requests, and then end the daemon. It takes no new request meanwhile.

---

<a id="01M7BF6GRG3NJZTA29TYVRVFT7"></a>

### Sessions, watching and concurrency

| Type | Design |
| --- | --- |
| ID | DES-DAEMON-SESSIONS |
| Status | Accepted |
| Satisfies | [REQ-NEVER-STALE](#01M7BF6GQSA67MYWBHWA6VYA3E) |
| Satisfies | [REQ-READS-DURING-VALIDATION](performance.mara.md#01M7BF6H3T3589901T1AE0ADHM) |

- One session per canonical project root. Git worktrees are separate projects.
- Watch mode is on by default. The daemon watches only directories that contain inputs ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)); inotify watch limits are per user.
- It debounces bursts such as a branch checkout and re-derives in the background. A watcher overflow or error triggers a full rescan.
- Readers take the current snapshot without locks. Writers queue per project and take the project's write lock ([DES-WRITE-PROCEDURE](writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)); there is no global lock.
- Long work runs on worker threads with cancellation. Protocol handlers never block each other.
- Caches live in memory.

---

<a id="01M7CQ57TBWFN78CCMEAY6XFTF"></a>

### Idle sessions

| Type | Design |
| --- | --- |
| ID | DES-IDLE-SESSIONS |
| Status | Draft |
| Decomposes | [DES-DAEMON-SESSIONS](#01M7BF6GRG3NJZTA29TYVRVFT7) |

- A session is dropped, with its snapshot and watches, after 30 minutes without a request. The number is provisional.
- A connected native client does not keep a session or the daemon alive. Its next call starts a daemon again ([REQ-FIRST-CALL-IN-PROCESS](#01M7BF6GQK5GM340Z7CP4C83B8)).
- A debug build takes the period in seconds from `MARA_TEST_IDLE_SECONDS` when it is set ([ADR-TEST-IDLE-PERIOD](#01M7JN4DSHNXYNDSQB08G5PPBF)). A release build ignores the variable and does not contain it ([DES-PERFORMANCE-HARNESS](performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)).

---

<a id="01M7CQ57TJY788DMPBN4BX1SEM"></a>

### Browser listener

| Type | Design |
| --- | --- |
| ID | DES-BROWSER-LISTENER |
| Status | Accepted |
| Satisfies | [REQ-BROWSER-ACCESS-AUTHORIZED](#01M7BF6GR0N9J4C8Z5QQ4CSCB7) |

- The listener is off unless `mara daemon start --http --allow-origin <origin> --allow-project <path>` starts it. Both options repeat, and no wildcard origin is accepted.
- The command prints the port and a token generated for that daemon run. `mara daemon status` prints them again.
- Nothing is persisted, and a daemon started on demand never listens on HTTP.
- A daemon that listens for browsers runs until `mara daemon stop`, because a browser cannot start one. Its idle sessions are still dropped.
- `--http` needs both options, and neither is accepted without it. An allowed origin is written as a browser sends it: `http` or `https`, a host and an optional port, with no path, no trailing `/` and no `*`; `null` and anything else are rejected with `invalid-argument`.
- The operating system assigns the port. The token holds at least 128 bits from the operating system's random generator.
- A request carries the token as `Authorization: Bearer <token>` and names its project root. It is refused unless the token matches, compared in constant time, its one `Origin` header equals an allowed origin byte for byte, and the project's canonical root is that of an `--allow-project` path. Token and origin are checked before the path is used, and the project before anything in it is read. A refusal opens no session, and has the same status and body whatever the path names.
- A response to an allowed origin carries `Access-Control-Allow-Origin` with that origin and `Vary: Origin`; a preflight from an allowed origin is answered without a token. A response to any other origin carries no `Access-Control-*` header.
- The listener serves the document operations that native clients send to the daemon. `project init` and `project convert` fail with `unavailable-operation`; nothing else is served.
- `mara daemon start --http …` while a daemon of the build runs prints that daemon's port and token and exits 0 when it listens with the same origins and projects, and otherwise fails with `daemon-running` and changes nothing.

---

## Decisions

---

<a id="01M7JN4DSHNXYNDSQB08G5PPBF"></a>

### Shorten the idle period in debug builds only

| Type | Decision |
| --- | --- |
| ID | ADR-TEST-IDLE-PERIOD |
| Status | Accepted |
| Justifies | [DES-IDLE-SESSIONS](#01M7CQ57TBWFN78CCMEAY6XFTF) |

- Chosen by the orchestrator on the maintainer's behalf on 2026-11-16: a debug build reads the idle period from `MARA_TEST_IDLE_SECONDS`, one variable for one period and no general test facility; a release build ignores it and does not contain it. The idle drop has to run in a test, and [ADR-DEBUG-PAUSE-POINT](writes.mara.md#01M7FCGJEWJ4EG946TYEVJAMW1) approved test-only code that exists in debug builds only.
- Rejected: a `--idle` option of `daemon start`. Users would see a capability made for a test.
- Rejected: inspection only. No test would exercise the idle drop.
