# Daemon verification

---

<a id="01M7JKW7JWB2RANYKNE6BHYM26"></a>

### Start and reach the daemon on demand

| Type | Verification |
| --- | --- |
| ID | VER-DAEMON-START |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-FIRST-CALL-IN-PROCESS](index.mara.md#01M7BF6GQK5GM340Z7CP4C83B8) |

Rust integration tests run the real CLI and `mara mcp` in a runtime directory of their own: a new directory of the test, mode 0700, set as `XDG_RUNTIME_DIR` on Linux and as `TMPDIR` on macOS for every client and every copy of the binary ([DES-DAEMON-LIFECYCLE](index.mara.md#01M7BF6GR80WA207P4BBFWW671)). Every other test that runs `mara` does the same or runs in direct mode, so no test reaches a daemon it did not start. A daemon of the test is a process of a binary under test whose environment holds that directory (`/proc/<pid>/environ` on Linux, `sysctl` `KERN_PROCARGS2` on macOS); tests identify and stop only those. The project's files last changed at least 2 s before its first call.

A call is served in-process when its stats line holds the counts of a cold request and no daemon of the test can have answered it: none runs or can start; or the only one runs a session of the project that answered a request sent at least 2 s after the last change, and would report nothing; or the first call that a daemon of the test answers afterwards reports the counts of a cold request too. A daemon reports the opening of a session with its first response for it ([DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA)), so it had answered nothing before. A call is served by the daemon when it repeats a request sent at least 2 s after the last change and its stats line holds those of the same request again ([VER-WORK-COUNTERS](../verification.mara.md#01M7BF6HGG6ET88HS641Y5676G)). A wait for a daemon polls `mara daemon status` for at most 10 s. From 0.4.0-alpha.6 the Node client of the setup of [VER-HOST-COMPARISON](../verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9) is a client too, with the stats that [DES-PERFORMANCE-HARNESS](../performance.mara.md#01M7BF6H4HKH0B0TXQZM3VXASA) gives it.

| Case | Expectation |
| --- | --- |
| No daemon runs; a read through the CLI, its stdout and stderr pipes | Served in-process; both pipes reach end of file when it exits; then `mara daemon status` reports a daemon with the project's session |
| The same read twice more, 2 s apart, once that session is open | The first reports the counts of a cold request, so the call before it was served in-process; the second is served by the daemon; no other daemon of the test starts |
| The test holds the build's daemon lock as a starting daemon holds it; a read through the CLI | Returns its result while the lock is still held, so that no daemon of the test can have answered it |
| Ten reads sent at once with no daemon running | Each returns the result of CLI direct mode; once all have exited and `mara daemon status` reports a daemon, it is the only daemon of the test |
| `CI` set, `MARA_NO_DAEMON=1` set, and the direct-mode flag, each with no daemon running and with one | Served in-process; no daemon of the test starts |
| With a daemon running, a read whose option's value is the direct-mode flag's text, `item list --id --no-daemon`; then `project validate -- --no-daemon` | The read is served by the daemon with the result of direct mode; the second is rejected with `invalid-argument` as in direct mode |
| The runtime directory's base naming no directory, or so long that the socket path would pass 108 bytes | Served in-process; no daemon of the test starts; nothing is created there |
| A base of 60 bytes | As with the test's usual directory |
| A daemon of the test killed with SIGKILL, leaving its socket | The next call is served in-process and starts a new daemon, which serves the calls after it |
| Copies of the binary at two paths, and one copy replaced in its place by a new copy while its daemon runs | Each copy serves its first call in-process and starts its own daemon: `mara daemon status` of each names another process |
| `mara mcp`, after the same request from the CLI settled a daemon of the test: one call | The server's stats line holds the counts of the same request again |
| One `mara mcp` server, started with no daemon running, sends the same read as four calls: call 1; once `mara daemon status` reports the session, 2 s later, call 2; then `mara daemon stop`; then call 3; once the session is reported again, 2 s later, call 4 | The server's stats line at exit is the sum of its calls' counts: call 1 + call 2 + call 3 + call 4 = 4 × the counts of a cold request. Call 1 and call 3 are served in-process, a cold request each, and each starts a daemon. Call 2 and call 4 are the first responses of those daemons, which report the opening of the session, a cold request each; the client adds nothing. Had call 1 or call 3 been answered by a daemon, the call after it would report nothing, and the total would be at most 3 × the counts of a cold request |
| From 0.4.0-alpha.6, a Node module of the test makes one read through the client of the package whose binary the CLI cases run, with no daemon running, and returns | Served in-process; the Node process exits within 5 s of the call settling; then `mara daemon status` reports a daemon with the project's session |
| From 0.4.0-alpha.6, the reads of the second case through the Node client | As there; `mara daemon status` names the process that served them: the client and the CLI of one package share their daemon |
| From 0.4.0-alpha.6, the Node client with `CI` set, with `MARA_NO_DAEMON=1` set and with its direct-mode option, each with no daemon running and with one | Served in-process; no daemon of the test starts |
| From 0.4.0-alpha.6, the Node client of a package whose binary is a copy at another path | Its first call starts the daemon of that copy: `mara daemon status` of each copy names another process |
| `mara daemon status`, `start` and `stop`, with no daemon running and with one | Exit status 0; `status` reports no daemon, or that daemon's process ID and sessions; after `start` exactly one daemon of the test runs; when `stop` returns, none runs |

---

<a id="01M7JKWYKZHG2JC489REG59HBM"></a>

### Answer from the files as they are

| Type | Verification |
| --- | --- |
| ID | VER-NEVER-STALE |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-NEVER-STALE](index.mara.md#01M7BF6GQSA67MYWBHWA6VYA3E) |

Rust integration tests start a daemon with its own runtime directory ([VER-DAEMON-START](#01M7JKW7JWB2RANYKNE6BHYM26)), send requests to it through the real CLI with `--format json`, and compare each result with that of the same request in CLI direct mode on the same files. In the first four cases the test makes each change with its own file operations and sends the requests at once, without waiting for the watcher, and in a second run 2 s later. That every request checks the inputs on disk whether or not the watcher reported a change is also inspected when mara-xqcnqw is reviewed: the cases below do not keep the watcher from seeing their changes.

| Case | Expectation |
| --- | --- |
| A document edited in place; saved by writing a new file and renaming it over the old one; added; added in a new directory; removed; renamed | `item list`, `search` and `project validate` return the results of direct mode |
| The schema edited; the configuration's include patterns edited; a `.gitignore` edited so that it ignores a document | The same |
| The documents' directory replaced by another, as a branch checkout replaces many files at once: documents changed, added and removed | The same |
| A program file edited | `project validate`, and `query execute` of a query of that program, return the results of direct mode |
| On the project of [VER-READS-DURING-VALIDATION](#01M7JKWYP3YSJGNEJJQ43RB13C) with a time limit of 5 s: a validation V1; once its `souffle` process runs, the long program replaced by a fast one with the same relations, and a validation V2 sent and answered while V1's process still runs; after V1 answers, a validation V3 | V1 returns the result of direct mode for the files before the replacement, with the time-limit fault; V2 and V3 return the result for the files after it, with neither V1's result nor its fault |
| A daemon started with `MARA_TEST_IDLE_SECONDS=4` ([DES-IDLE-SESSIONS](index.mara.md#01M7CQ57TBWFN78CCMEAY6XFTF)), with sessions of projects A and B and a `mara mcp` server of the test connected; a request to A every second for 8 s and none to B; then B's next request; then no request | After the 8 s, `mara daemon status` lists A's session and not B's; B's request returns the result of direct mode with the counts of a cold request; within 10 s of the last request no daemon of the test runs, and the server's next call is served in-process |
| Two copies of one project, as two Git worktrees are, with the same relative paths and other contents; requests alternating between them | Each result is that of direct mode for its own copy |
| A read while an `item move` of a daemon started with `MARA_TEST_PAUSE` waits in step 3 of its second file ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) | The result of direct mode for the files as they are then, the destination already written |

---

<a id="01M7JKWYP3YSJGNEJJQ43RB13C"></a>

### Read while a validation runs

| Type | Verification |
| --- | --- |
| ID | VER-READS-DURING-VALIDATION |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-READS-DURING-VALIDATION](../performance.mara.md#01M7BF6H3T3589901T1AE0ADHM) |

Rust integration tests start a daemon with its own runtime directory ([VER-DAEMON-START](#01M7JKW7JWB2RANYKNE6BHYM26)) and a `TMPDIR` under the test's temporary directory, which on macOS is that runtime directory, on a project whose invariant uses the long program of [VER-EVALUATION-TIME-LIMIT](../rules/verification.mara.md#01M7HZYKBNDY3ZA77JMB0VD06H) with a time limit of 60 s set in the configuration. A validation runs once a `souffle` process with its working directory under that `TMPDIR` exists; the test then sends the reads, and stops the daemon at the end. The warm budget is checked nightly by [VER-TIME-BUDGETS](../verification.mara.md#01M7BF6HH06WEJ67P04H22BM66), Draft with [REQ-TIME-BUDGETS](../performance.mara.md#01M7BF6H47MJ8H16R3B4S7750T).

| Case | Expectation |
| --- | --- |
| `project validate` through the CLI; while it runs, `item list`, `get` of an item and `search` through other CLI calls | Each read returns the result of CLI direct mode while that `souffle` process still runs and before the validation answers |
| One `mara mcp` session that sends a `project_validate` call, then those reads as calls while it runs | The same |
| `item list` while an `item create` of a daemon started with `MARA_TEST_PAUSE` waits before step 2 ([DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17)) | Returns the result of direct mode for the files before the create, while the create still waits |

---

<a id="01M7JKWYR9HA4N12DQCA5GMRRR"></a>

### Write through the daemon

| Type | Verification |
| --- | --- |
| ID | VER-DAEMON-WRITES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-WRITERS-SERIALIZED](../authoring/writes.mara.md#01M7CXC2N8EZYVHGY7T2GPPB3X) |
| Verifies | [REQ-REFUSED-EDIT-CHANGES-NOTHING](../authoring/writes.mara.md#01M7BF6HHTMM6D6QAF0BHFBCE6) |
| Verifies | [REQ-INTERRUPTED-EDIT-LOSES-NOTHING](../authoring/writes.mara.md#01M7BF6HJ3758Y06V61901ZSKK) |

Rust integration tests write through the real CLI to a daemon that the test starts with `mara daemon start` and its own runtime directory ([VER-DAEMON-START](#01M7JKW7JWB2RANYKNE6BHYM26)). They set the pause points of [DES-WRITE-PROCEDURE](../authoring/writes.mara.md#01M7BF6HJA1E3SE8WJH77FGP17) by starting the daemon with `MARA_TEST_PAUSE`.

| Case | Expectation |
| --- | --- |
| Ten `item create` calls with distinct titles sent at once to one document | Every call succeeds and its item is in the document; the project gains no diagnostic |
| Another program saves a document with an added item; at once, an `item update` of another item of that document | The document holds the added item and the update |
| The target of an `item update` changed by the test while the daemon's writer waits before step 2; then the same update sent again | The first fails with `changed-file`, and every file is as the test left it; the second succeeds on the changed contents |
| `mara daemon stop`, and in a second run SIGTERM to the daemon, while an `item move` waits in step 3 of its first file; then the pauses released | The move's client reports success, and the files are those of the completed move; then the daemon exits |
| SIGKILL to the daemon while an `item create` waits before step 2 | The client fails with `daemon-lost` and exit status 1 and does not run the create itself: every file is byte-identical |
| SIGKILL to the daemon while an `item move` waits in step 3 of its second file | The client fails with `daemon-lost`; the files hold an interrupted move as [VER-INTERRUPTED-EDITS](../authoring/verification.mara.md#01M7F74J3SE8HJ3S6ZNG3G2DYJ) allows it, which the next `project validate`, served in-process, reports |
| From 0.4.0-alpha.6, the two SIGKILL cases with the write sent through the daemon by the Node client | The call fails with `daemon-lost`, and the files are as in those cases: the client does not run the write again in-process |

---

<a id="01M7JQJKGBQKMRHH97R3RRJJ62"></a>

### Refuse every unauthorized browser request

| Type | Verification |
| --- | --- |
| ID | VER-BROWSER-ACCESS |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-BROWSER-ACCESS-AUTHORIZED](index.mara.md#01M7BF6GR0N9J4C8Z5QQ4CSCB7) |

Rust integration tests start each daemon from the test's temporary directory, outside every project, with its own runtime directory ([VER-DAEMON-START](#01M7JKW7JWB2RANYKNE6BHYM26)), and an HTTP client of the test sends HTTP/1.1 requests to `127.0.0.1` at the port that daemon printed. The listening daemon is started with `--http --allow-origin http://127.0.0.1:5173 --allow-origin http://127.0.0.1:5175 --allow-project <A> --allow-project <C>`; B is a third disposable project, and the documents of each project hold a word of their own. An accepted request carries the token, `Origin: http://127.0.0.1:5173` and project A, and is a read or a write; each case changes one part of it. A refusal is a 4xx status whose status and body are those of the same request naming a path where nothing exists, and hold no project's word; to a request whose one `Origin` header is an allowed origin it carries the CORS headers of an accepted response, and to any other request no `Access-Control-*` header. After a refusal, `mara daemon status` lists no session that the daemon did not hold before, and every file of A, B and C is byte-identical ([DES-BROWSER-LISTENER](index.mara.md#01M7CQ57TJY788DMPBN4BX1SEM)).

| Case | Expectation |
| --- | --- |
| `mara daemon start --http` without `--allow-origin`, and without `--allow-project`; either option without `--http`; the origins `*`, `null`, `http://*.example.com`, `http://127.0.0.1:5173/`, `127.0.0.1:5173` and `file:///` | Rejected with `invalid-argument`, exit status 2; no daemon of the test runs |
| A daemon started on demand by a CLI read; one started by `mara daemon start` | The daemon process has no listening TCP socket: none of its socket inodes in `/proc/<pid>/fd` is in the listening state in `/proc/net/tcp` or `/proc/net/tcp6` (on macOS, `lsof -a -p <pid> -iTCP -sTCP:LISTEN` lists none); `mara daemon status` prints no port or token |
| The listening daemon | `start` prints a port and a token, which `status` prints again; each listening socket of the daemon is bound to `127.0.0.1` or `::1`; the accepted read and write return the result of CLI direct mode, with `Access-Control-Allow-Origin: http://127.0.0.1:5173` and `Vary: Origin` |
| The accepted read from each allowed origin to each allowed project: four requests | Each returns the result of CLI direct mode for its project, with `Access-Control-Allow-Origin` naming its own origin |
| An `OPTIONS` preflight without a token, asking for the method and headers of the accepted request, from the allowed origin and from `http://127.0.0.1:5174` | The first is allowed them for that origin; the second gets no `Access-Control-*` header; neither opens a session |
| The token left out, empty, with one character changed, in the query string instead of `Authorization`; the token of the previous run after `mara daemon stop` and a new start | Refused |
| `Origin` left out; given twice, once allowed; `null`, `https://127.0.0.1:5173`, `http://localhost:5173`, `http://127.0.0.1:5174`, `http://127.0.0.1:51730`, `http://127.0.0.1:5173.example.com` and `http://127.0.0.1:5173/` | Refused |
| Project B; no project; `<A>/../<B>`; a symbolic link in A to B; a project nested in a directory of A; a regular file of A | Refused |
| A symbolic link outside A to A | Served as A |
| A write to A at the document path `../<B>/docs/x.mara.md` | Fails with `not-an-input`, as in CLI direct mode; B is byte-identical |
| `project init` and `project convert` of A; requests naming `daemon stop` and `code add` | Fail with `unavailable-operation`; nothing is written in A or in the grammar cache, and the daemon still runs |
| While the listening daemon runs: `mara daemon start` with the same listener options | Exit status 0, printing the same port and token |
| While the listening daemon runs: `mara daemon start` with a third `--allow-origin`; with `--allow-project <B>` added; with `--allow-project <C>` left out | Each fails with `daemon-running`, exit status 1; then `status` prints the same port and token, a request from the third origin and one for B are refused, and the four accepted reads above still return their results |
| While a daemon started on demand runs: `mara daemon start` with the listener options | `daemon-running`; that daemon still has no listening socket |
| The listening daemon started with `MARA_TEST_IDLE_SECONDS=4`; an accepted read, then no request for 10 s, then the read again | After the 10 s it still runs and `status` lists no session; the second read returns the result of CLI direct mode |
| `mara daemon stop` | Once it returns, a connection to the port is refused; the token is in no file under the runtime directory, the test's `HOME` or its `TMPDIR`; the next daemon started on demand has no listening socket |
| In the browser setup of [VER-HOST-COMPARISON](../verification.mara.md#01M7D3NAQHP8HX0X0GCZMXD1Q9), two pages served by the test from two loopback ports, the daemon allowing only the first's origin: the second page, then the first, `fetch` a read of A with the token | The second's `fetch` rejects and `status` lists no session of A; the first gets the result of CLI direct mode |

Inspection: review of mara-fkkcad finds the token drawn from the operating system's random generator with at least 128 bits, compared in constant time, and the token and the origin checked before the request's path is used. A test observes neither a token's entropy nor the timing of a comparison.
