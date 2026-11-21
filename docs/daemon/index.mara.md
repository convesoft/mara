# Daemon

| Page | Holds |
| --- | --- |
| [Daemon verification](verification.mara.md) | Checks of daemon start, fresh answers, reads during validation, writes through the daemon and browser access |

## Requirements

---

<a id="01M7BF6GQK5GM340Z7CP4C83B8"></a>

### The first call never waits for the daemon

| Type | Requirement |
| --- | --- |
| ID | REQ-FIRST-CALL-IN-PROCESS |
| Status | Accepted |
| Satisfies | [NEED-INTERACTIVE-SPEED](../product.mara.md#01M7BF6H83C0SM1C8RHDHMBXZ2) |

A CLI, MCP or Node client that finds no daemon starts one in the background and runs its current request in-process. Later calls use the daemon.

---

<a id="01M7BF6GQSA67MYWBHWA6VYA3E"></a>

### Stale state is never returned as current

| Type | Requirement |
| --- | --- |
| ID | REQ-NEVER-STALE |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](../product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

Every request checks the declared inputs on disk. When one has changed, the response waits for the rebuild.

---

<a id="01M7BF6GR0N9J4C8Z5QQ4CSCB7"></a>

### Browsers reach the daemon only when authorized

| Type | Requirement |
| --- | --- |
| ID | REQ-BROWSER-ACCESS-AUTHORIZED |
| Status | Accepted |
| Satisfies | [NEED-ONE-BEHAVIOUR-EVERYWHERE](../product.mara.md#01M7BF6H7Q0YP8SPVYWYJFB1MF) |

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

- Each Mara build has its own lock and socket in the runtime directory (`mara/`, mode 0700, in `XDG_RUNTIME_DIR`, or `/run/user/<uid>` when it is unset, on Linux; in `TMPDIR` on macOS). A build is its version with the path, size and modification time of its executable ([ADR-DAEMON-PER-VERSION](#01M7CMW6Z9MMB1705N02S9CCYG)). The lock ensures one instance per build when several clients start it at once; the socket path, with its terminating NUL, fits `sun_path`: 108 bytes on Linux, 104 on macOS.
- When that base names no directory, or the socket path would pass the limit, every call runs in direct mode.
- The base must be absolute, a directory and no symbolic link, owned by the user with mode exactly 0700, special bits included, or every call runs in direct mode too. `mara/` must be the same, or the `daemon` operations fail with `io-error` naming it; nothing in it is changed or followed.
- The lock is `<key>.lock` and the socket `<key>.sock`, `<key>` being the first 16 hexadecimal digits of the BLAKE3 hash of the build. The lock is a regular file of mode exactly 0600, whatever the umask: it is created under its own name, with the process ID and a number the process never gives twice, given mode 0600 and then linked into place, so it never appears with another. It is opened without following a link, and one of another mode or owner is refused, never repaired. The daemon holds it with `flock` until it exits and writes its process ID into it. The socket is bound with mode 0600, whatever the umask, and a client refuses one of another mode or owner. Only the holder of the lock removes a stale socket, and only a socket; a start that fails after binding its socket removes it.
- A frame is a 4-byte big-endian length and a JSON object of at most 64 MiB, with `version` 1. A request carries the client's build key; the daemon refuses another. Daemon and client each drop a peer of another effective user ID (`SO_PEERCRED`, `getpeereid`).
- A client connects only to the daemon of its own build. Daemons of different builds run side by side. A Node client's build is that of the `mara` binary of its native package, which it starts as its daemon.
- Clients send every document operation to the daemon except `project init` and `project convert`, which run in the calling process, as `prime` does.
- A client that finds no daemon of its build starts `mara daemon start`, with its project as `--project`, detached from its standard streams, and serves its request in-process. The start runs in the client's working directory and in a process group of its own, with the client's `--project` made absolute when it has one; the client does not wait for it, and a start that fails does not fail the request. A daemon opens the session of the project that `--project` names or that holds its working directory, if any, as soon as it starts, and derives it in the background ([DES-DAEMON-SESSIONS](#01M7BF6GRG3NJZTA29TYVRVFT7)).
- A client falls back to direct mode only before it dispatches a request. A dispatched request is never replayed on another backend: when the daemon ends before answering, the request fails with `daemon-lost` ([DES-OPERATIONS](../hosts/index.mara.md#01M7BF6H124JRJAQSYVQ21Q2JS)).
- A client connects without blocking: while the daemon's queue of connections is full it tries again for at most 2 s, then serves its request in-process and starts no daemon. It hands the request over within 10 s in all, or serves it in-process.
- While it runs an operation request, the daemon sends the client a heartbeat frame (`working` true) every second. A client waits as long as frames come; after 15 s without one the request fails with `daemon-lost`, so a long request is never cut off and a daemon that stopped or hangs never holds a client for longer.
- A request is dispatched once its frame is written in full. A daemon that refuses a request, because it stops, serves another build or finds the request malformed, ran nothing, and the client serves the request in-process. A client whose runtime directory or socket fails the checks above serves its request in-process and starts no daemon.
- An operation request carries the operation's words, its arguments and options as the client checked them, the inputs of an MCP call, `--project`, whether the reply is JSON, and these values of the client's process, which the operation uses instead of the daemon's: the working directory; the umask; the temporary directory (`TMPDIR`, or `/tmp` when it is unset, made absolute). An operation that reads another value of its process or environment adds it to this list. The daemon checks the arguments again and resolves `--project` and every file an argument names, such as `--input <name>=@<file>`, from the client's directory; it runs in `/` and never resolves a path from its own. It answers with the text the client prints, whether it is a failure, the exit status and the work counts of the request's session since its previous response ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)), which the client reports as its own.
- Every file and directory that a request served by the daemon creates, a rule evaluation's private directory and the files Soufflé writes included, gets the mode that the client's umask gives it in direct mode; an existing file keeps its mode, as in direct mode. A file written whole is first created exclusively and opened as an existing file only when that finds one, so a symbolic link to nothing there, or a file removed between the two opens, fails the write with not-found and nothing is created. The daemon never changes its own umask, which the requests it serves at once share. A client reads its umask without changing it, from `Umask:` of `/proc/self/status` on Linux. Where that file cannot be read or has no such line (Linux before 4.7, no `/proc`) and on macOS, which offers no other read, it sets the umask and sets it back, so a file another thread of the client's process creates in between gets its mode bits less 077. Rule evaluations run in the client's temporary directory.
- The daemon exits when its last session is dropped ([DES-IDLE-SESSIONS](#01M7CQ57TBWFN78CCMEAY6XFTF)), or after the idle period when it opened none, unless it listens for browsers ([DES-BROWSER-LISTENER](#01M7CQ57TJY788DMPBN4BX1SEM)). Without a session, the idle period is that of [DES-IDLE-SESSIONS](#01M7CQ57TBWFN78CCMEAY6XFTF), counted from its start or its last request; `status` and `stop` requests count as none, so a client watching the daemon does not keep it alive.
- A client runs in direct mode, neither connecting to a daemon nor starting one, when `CI` is set to a non-empty value, `MARA_NO_DAEMON` is `1` or the direct-mode flag `--no-daemon` is given; `mara mcp --no-daemon` serves every call in-process.
- `mara daemon status | start | stop` give explicit control. `status` prints whether the daemon of the client's build runs, its process ID, the root of each open session, and how many operation requests it has run and answered since it started, counted before each answer is sent. `start` and `stop` exit 0 also when there is nothing to do; `stop` returns once the daemon has exited. `CI` and `MARA_NO_DAEMON` do not affect them. Without a runtime directory, `start` fails with `io-error`, and `status` and `stop` report that no daemon runs.
- `status --format json` prints `running`, `pid`, `sessions`, `served`, the count of operation requests, `socket`, `lock` and `unavailable`, the reason no daemon can run, or null for each that does not apply.
- `daemon start` takes the lock and forks the daemon, which keeps it, starts a new session (`setsid`), points its standard streams at `/dev/null`, closes every other descriptor it inherited, and listens; `start` then returns. While another process holds the lock, `start` waits up to 10 s for a daemon of its build to answer, then fails with `io-error` naming the lock.
- `mara daemon stop` and SIGTERM, SIGINT or SIGHUP let the writes the daemon runs finish, end its other requests, and then end the daemon. It takes no new request meanwhile. It answers `stop` before it begins to stop, so the answer always comes before the process exits; a client whose connection closes after it asked for the stop reports the daemon stopped. Its rule evaluations end through their own cancellation, each killing its Soufflé child and removing its private directory before the daemon exits; a request ended so fails with `daemon-lost`. An evaluation is counted before it checks for the stop, and the daemon sets the stop before it reads the count: so either the daemon waits for the evaluation, or the evaluation sees the stop before it creates its private directory and creates none.

---

<a id="01M7BF6GRG3NJZTA29TYVRVFT7"></a>

### Sessions, watching and concurrency

| Type | Design |
| --- | --- |
| ID | DES-DAEMON-SESSIONS |
| Status | Accepted |
| Satisfies | [REQ-NEVER-STALE](#01M7BF6GQSA67MYWBHWA6VYA3E) |
| Satisfies | [REQ-READS-DURING-VALIDATION](../performance.mara.md#01M7BF6H3T3589901T1AE0ADHM) |

- One session per canonical project root. Git worktrees are separate projects.
- A session keeps the project's tracked inputs ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)) and the work done since its previous response, which that response reports ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)). A request selects its project from its working directory and `--project` and runs in that project's session, opened unless it is open; a request that selects no project runs in none.
- The code files that a request reads, every one for `project validate` or only those a program over code links reads ([DES-CODE-READERS](../code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)), are inputs of the session like the documents: listed, read and scanned through the session's inputs, each read again only when its stat changed and scanned again only when its contents, language or grammar changed. A request lists them under the session's lock, then reads them, loads grammars and parses without it, so that no other request waits for a scan; it gives back only the results it read itself, judged against the record it started from, each only while its file still has the stat it was read with, and keeps its own results for its edit. It compares a result with the session's record at the moment it gives back only to skip one that the session already holds as it is: a scan that read nothing and found no file gone changes nothing of the session, and a result it only reused never brings back a file that a newer listing dropped, while a file it read itself comes back until the next scan. A code file that a request's listing of the code files no longer finds as one (deleted, renamed, ignored, no longer matched by an include pattern, or now a document or a program), or at whose path `get` finds no code file, loses the contents and grammar-level scan that `get` kept, and the next scan drops it from the session's record of scanned files after reusing by key the scan of a file that was only renamed: a scan drops only paths of the record it started from, so a file that a newer scan added stays; decided during the rewrite on 2026-11-21 (mara-agdnwh). `get` of a code location checks only the directories on the way to its path ([DES-CODE-READERS](../code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)), so what `get` kept of another file that left stays until the next listing of the code files or `get` of that path: it costs memory, never a stale answer, since every answer checks the file's stat. The listings and `.gitignore` files that `get` lists or reads are kept in the session apart from the inventory, so that a background derivation or a request that lists no code does not drop them, and they are not watched. Every walk of the inputs takes a record from there where it is the one a check keeps, and gives the directories it lists their new records there; a listing of the code files, which reaches every directory that may hold one, also drops the directories it does not list. A walk chooses a record of a directory or `.gitignore` only where it reaches it, after the directory's parent admitted it, from the metadata it took there, so no record is checked through a symbolic link; decided during the rewrite on 2026-11-21 (mara-h4z424). The one code file that `get` of a code location reads is checked against the contents the session keeps of it, from an earlier `get` or from a scan, and is scanned at the grammar level only when the session keeps no scan of the same contents with the same grammar; a scan that fell back is not kept; decided during the rewrite on 2026-11-21 (mara-p27z9t). Of the records the session keeps of one file or directory, from an earlier `get`, a scan or the inventory, the one that a check keeps answers: its stat is the file's and its check settled it ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)); when none does, the file is read or the directory listed again. No record is chosen by its check time; decided during the rewrite on 2026-11-21 (mara-h4z424). A scan keeps the contents only of a code file with markers, so the first `get` of a code file without markers after a scan reads and hashes it once: keeping every scanned file's contents would hold the whole code base in the session's memory. Decided by the orchestrator on the maintainer's behalf on 2026-11-21, provisional (mara-2ndg14).
- Every request checks the inputs on disk, whatever the watcher delivered. Watching only moves the derivation out of the request. Requests are told apart by an identity of their own, never by their time: a clock that stands still or steps back makes no input look checked.
- Watch mode is on by default. The daemon watches only directories that contain inputs ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)): those the last inventory listed, `.mara` and the schema's directory; inotify watch limits are per user.
- Linux: one inotify instance per daemon, through `libc`, with no further dependency. A directory that cannot be watched, past the per-user limit for instance, is checked by each request all the same. Other systems do not watch yet: every request still checks the inputs on disk.
- A sync of a session's watches looks up directories without the watcher's lock, and takes it for each change of a watch: one `inotify_add_watch` or `inotify_rm_watch` with its record, the lock released before the next. The thread that reads events and other sessions wait for at most one such call, and no watch is recorded after it was removed. A request's sync registers its session for the root; before each change a sync checks that its session is still the one registered, so a background sync of a session that was forgotten or replaced changes nothing more.
- Watching leaves gaps, which requests close by checking the inputs on disk: the watcher misses a change in a directory before its watch is added, and a directory replaced between its lookup and its watch is watched again only at the next sync. A watch replaced while another sync of the same root runs can also be left unreleased (mara-es7j9r).
- A watch is on a directory's inode. One whose directory was deleted or moved away, or whose path names another directory at a later check, as after a branch checkout, is replaced by a watch of what the path names then. A session that `daemon start` opens is watched from its first derivation.
- It debounces bursts such as a branch checkout and re-derives in the background: 50 ms after the last change of a burst, at most 500 ms after its first. A watcher overflow or error triggers a full rescan: every session re-derives, which checks every input.
- A background derivation does what a request that reads documents does, at most one per session at a time; it runs again when a change came meanwhile. Its work goes to the session's next response. It does not read again an input, the configuration and the schema included, whose stat is that of its last check and that a check now would still leave unsettled ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)): such a read cannot settle the file, and the next request's check reads it all the same. It adds its work to the next response before it gives what it derived to other requests, so the response of a request that uses that state reports that work.
- A file that the watcher's derivation read less than 2 s after its mtime or ctime is read and hashed again by the next request's check, which settles it once 2 s have passed since the change: a watch is never taken as proof that a file is unchanged. Decided by the orchestrator on the maintainer's behalf on 2026-11-21 (mara-8hw038).
  - Rejected: trusting a watch in place before the check. Inotify misses edits that keep the stat: through a hard link in an unwatched directory, from another machine on a network file system, through a memory mapping in the same timestamp tick.
  - Rejected: deferring the derivation until the changed files settle. Requests in the first 2 s after an edit would lose the warm start that the 50 ms debounce gives.
- Readers take the current snapshot without locks. A request holds the session's inputs only while it checks and derives them, so a check waits only for a check in progress, and then keeps what its own check found: an edit is computed and checked against those contents, whatever later requests find. Writers queue per project and take the project's write lock ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)): that lock is the queue, as for writers of other processes; there is no global lock.
- After its renames, a write served in a session gives the session each document it wrote, with the read-back's parse ([ADR-READ-BACK-EVERY-EDIT](../authoring/index.mara.md#01M7FH6HEWNYQ66F5KE912BQSM)) and the stat taken after the rename, and derives the snapshot from them ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) step 4): one hash per file written, of the contents it wrote, and no read or other parse; every other input whose stat is that of its last check is kept without reading, since the write changed none. From the write's first rename to that hand-over, no background derivation of the session runs: one in progress is waited for, and the next waits for the hand-over; reads never wait for it. Each such file counts as checked when the request began, before the renames, so the next check reads and hashes it again until a check settles it, and a change that another program made after the rename is never taken for the written contents.
- A file that `--input` names ([DES-REQUEST-INPUTS](../rules/index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)) is no input of the session: every request that names it reads it, and the session keeps nothing of it, so its answer reflects the file as it is. Decided by the orchestrator on the maintainer's behalf on 2026-11-21 (mara-p27z9t).
  - Rejected: tracking request-supplied files in the session. The caller names the file afresh with each request, and it may lie outside the project and its watches.
- Long work runs on worker threads with cancellation. Protocol handlers never block each other: each connection has its thread, and each operation request runs on a worker thread of its own while the connection's thread sends the heartbeats of [DES-DAEMON-LIFECYCLE](#01M7BF6GR80WA207P4BBFWW671). A stopping daemon ends running rule evaluations through their cancellation: each kills its Soufflé child and removes its private directory.
- Caches live in memory. A session keeps the findings of the built-in checks with the snapshot they checked, so a request on the same snapshot runs no check step; decided during the rewrite on 2026-11-21 (mara-p27z9t). A session keeps the outcomes of its evaluations by key ([DES-RULE-EXECUTION](../rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)): a request whose evaluation an earlier request of the session ran with the same key runs no program. Only rows and compile errors are kept, never a stop at a limit, a runtime fault or a fault of the host, which may depend on the machine at the time. An outcome is kept for the Soufflé executable that gave it, known by its path, size and modification time as a daemon's build is ([DES-DAEMON-LIFECYCLE](#01M7BF6GR80WA207P4BBFWW671)), so a Soufflé replaced by one of another path, size or modification time evaluates again, and one with all three the same does not; decided by the orchestrator on the maintainer's behalf on 2026-11-21. A session keeps the outcomes of its runs in provenance mode (`query report`) under the same rules, apart from evaluations, by key, the Soufflé executable's path, size and modification time, and the rows explained: a report reads a kept run of its key and Soufflé that explained the rows it chooses from that run's rows, and judges its outcome from those rows itself: a report with other inputs, or of another item or entry that chooses other rows, runs; one that differs only in its page options, or whose entry of the same program chooses the same rows, does not. Only a run whose every chosen row has the explanation, or the reason it has none, that the exchange gave, and that then ended on its own with a successful exit, at no limit and with nothing more on its standard error, is kept, or a compile error; never one that left a row unexplained at the time limit, an abort, a stray or invalid reply or past 16 MiB of replies, nor one stopped at a limit or failed after its last reply, whose response still keeps its proofs ([DES-RULE-EXECUTION](../rules/evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)). A stray reply that comes after every chosen row was explained leaves the run kept. The rest of what an explanation depends on is in the key (the facts with their locations, the supplied inputs, the program, the limits) or fixed by the daemon's build (the bounds on a proof). Decided during the rewrite on 2026-11-21 (mara-6za81b). A session keeps at most 64 MiB of outcomes of both kinds, each estimated at 256 bytes plus its rows, and its rows explained and explanations, the oldest dropped first; the numbers are provisional. An evaluation runs without holding the store, so no request waits for another's evaluation, and two requests at once may each run it.

---

<a id="01M7CQ57TBWFN78CCMEAY6XFTF"></a>

### Idle sessions

| Type | Design |
| --- | --- |
| ID | DES-IDLE-SESSIONS |
| Status | Draft |
| Decomposes | [DES-DAEMON-SESSIONS](#01M7BF6GRG3NJZTA29TYVRVFT7) |

- A session is dropped, with its snapshot and watches, after 30 minutes without a request. The number is provisional.
- The period runs from the start or end of the session's last request, or from its opening; a session that serves a request is not dropped. The daemon exits once it has dropped its last session and serves no request.
- A connected native client does not keep a session or the daemon alive. Its next call starts a daemon again ([REQ-FIRST-CALL-IN-PROCESS](#01M7BF6GQK5GM340Z7CP4C83B8)).
- A debug build takes the period in seconds from `MARA_TEST_IDLE_SECONDS` when it is set ([ADR-TEST-IDLE-PERIOD](#01M7JN4DSHNXYNDSQB08G5PPBF)). A release build ignores the variable and does not contain it ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)).

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

- Chosen by the orchestrator on the maintainer's behalf on 2026-11-16: a debug build reads the idle period from `MARA_TEST_IDLE_SECONDS`, one variable for one period and no general test facility; a release build ignores it and does not contain it. The idle drop has to run in a test, and [ADR-DEBUG-PAUSE-POINT](../authoring/writes.mara.md#01M7FCGJEWJ4EG946TYEVJAMW1) approved test-only code that exists in debug builds only.
- Rejected: a `--idle` option of `daemon start`. Users would see a capability made for a test.
- Rejected: inspection only. No test would exercise the idle drop.

---

<a id="01M7BF6GS1109M3YV9WHTT4XTK"></a>

### Keep caches in memory

| Type | Decision |
| --- | --- |
| ID | ADR-MEMORY-ONLY-CACHES |
| Status | Accepted |
| Justifies | [DES-DAEMON-SESSIONS](#01M7BF6GRG3NJZTA29TYVRVFT7) |

- Chosen: no on-disk store. Documents and code markers rebuild in milliseconds to seconds, so a restart is cheap.
- Rejected until a measured cost requires it: an on-disk store.

---

<a id="01M7BF6GS99YFG0VNDCJ1QQRCB"></a>

### Start the daemon on demand and watch by default

| Type | Decision |
| --- | --- |
| ID | ADR-ON-DEMAND-DAEMON |
| Status | Accepted |
| Justifies | [DES-DAEMON-LIFECYCLE](#01M7BF6GR80WA207P4BBFWW671) |
| Justifies | [DES-DAEMON-SESSIONS](#01M7BF6GRG3NJZTA29TYVRVFT7) |

- Chosen: clients start the daemon in the background; it watches declared inputs, so the next request is already warm, and exits when idle.
- Rejected: explicit start only, as in 0.4.0-alpha.1. The CLI never started a daemon, so most calls ran cold.

---

<a id="01M7CMW6Z9MMB1705N02S9CCYG"></a>

### Run one daemon per Mara version

| Type | Decision |
| --- | --- |
| ID | ADR-DAEMON-PER-VERSION |
| Status | Accepted |
| Justifies | [DES-DAEMON-LIFECYCLE](#01M7BF6GR80WA207P4BBFWW671) |

- Chosen: each version has its own lock and socket. Daemons of different versions coexist and exit when idle.
- Rejected: one daemon per OS user that exits when a client of another version arrives. Projects pinned to different versions would stop each other's daemon on every call and never run warm.
- Chosen by the orchestrator on the maintainer's behalf on 2026-11-16: a daemon belongs to a build, its version with the path, size and modification time of its executable, so it never answers for another build's code ([REQ-SAME-RESULT-ON-EVERY-HOST](../engine.mara.md#01M7BF6GXHZNABX9R7NKPNGJNM), [REQ-NEVER-STALE](#01M7BF6GQSA67MYWBHWA6VYA3E)).
- Rejected: the version alone. Several worktrees of one version run different code every day.
- Rejected: a hash of the executable's contents. Every call would read the whole binary.

---

<a id="01M7CQ57T4TYP96W06VB9P2W96"></a>

### Enable browser access only by flags on `daemon start`

| Type | Decision |
| --- | --- |
| ID | ADR-HTTP-BY-FLAG |
| Status | Accepted |
| Justifies | [DES-BROWSER-LISTENER](#01M7CQ57TJY788DMPBN4BX1SEM) |

- Chosen: the user turns the listener on and names origins and projects when starting the daemon. Nothing is persisted.
- Rejected: allowed origins in the project configuration. A cloned repository could grant a web origin access to itself.
