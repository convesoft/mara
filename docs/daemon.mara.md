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

- Each Mara version has its own lock and socket in the runtime directory (`$XDG_RUNTIME_DIR/mara/`, mode 0700). The lock ensures one instance per version when several clients start it at once; the socket path stays under the 108-byte Unix limit.
- A client connects only to the daemon of its own version. Daemons of different versions run side by side.
- A client falls back to direct mode only before it dispatches a request. A dispatched request is never replayed on another backend.
- The daemon exits when its last session is dropped ([DES-IDLE-SESSIONS](#01M7CQ57TBWFN78CCMEAY6XFTF)), unless it listens for browsers ([DES-BROWSER-LISTENER](#01M7CQ57TJY788DMPBN4BX1SEM)).
- No client starts a daemon when `CI` is set, `MARA_NO_DAEMON=1` is set or the direct-mode flag is given.
- `mara daemon status | start | stop` give explicit control.

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
