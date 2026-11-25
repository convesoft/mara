# Rules decisions

---

<a id="01M7BF6GSZ16MZQAGBY5M9TDKZ"></a>

### Write project logic in Soufflé Datalog

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-DATALOG |
| Status | Accepted |
| Justifies | [DES-RULE-CONSUMERS](index.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE) |

- Chosen: Soufflé. A program declares its typed input and output relations, mistakes are rejected before evaluation, and modules are native (`.include`, `.once`, `.comp`).
- Fallback if Soufflé becomes unworkable: Nemo. Not chosen first because it is dynamically typed and silently skips mismatching rows.
- Rejected: GraphQL. Computed fields need custom resolvers or Mara-specific directives.
- Rejected: SPARQL. It has no modules or typed input and output declarations, so it needed a JSON wrapper plus separate SHACL constraints.
- Rejected: Rego (`regorus`). It forbids recursive rules.

---

<a id="01M7CQ57SYJ9BH5XMVVV9F6QA3"></a>

### Register invariants, gates and views in the configuration

| Type | Decision |
| --- | --- |
| ID | ADR-CONSUMERS-IN-CONFIGURATION |
| Status | Accepted |
| Justifies | [DES-RULE-CONSUMERS](index.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE) |

- Chosen: named configuration entries that point at a program's output relations. Programs stay plain Soufflé.
- Rejected: output relations that register themselves by name, such as `invariant_error_<name>`. Names would carry hidden meaning, and a message has no place.
- Rejected: reaching any output relation by program file and relation name. Helper relations would become public, without a description.

---

<a id="01M7BF6GT9PVXSKV1ANGEWA7WR"></a>

### Run Soufflé through its command line

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-AS-PROCESS |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |

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
| Justifies | [DES-PROGRAM-PROFILE](index.mara.md#01M7CMW7051QVPFDWQBMB1A6K6) |
| Justifies | [REQ-EVALUATION-TIME-LIMIT](evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0) |

- Chosen: Mara parses every program against the grammar of a small admitted language, with types resolved, and runs stock Soufflé 2.5 only on programs that pass, on parsed copies in a private directory with library loading off; a time limit bounds each evaluation ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).
- Rejected: lists of refused or admitted words. Five reviews each found ways past one, and a word list cannot express literal types, casts or column types.
- Rejected: an evaluator inside the engine for the same language. It has no trial behind it and would reverse the choice of Soufflé; it stays the alternative if this boundary keeps failing.
- Rejected: patching Soufflé or tracking its master. The behaviours left out are by design upstream or differ between the two builds, and a fork must be verified again on every update.
- Rejected: trusting programs like build scripts. Validating an untrusted checkout would run its code.
- Rejected: forbidding records to bound evaluation. Plain recursion with arithmetic or `cat` never ends either.

---

<a id="01M7CT60PG5Q68ZNJ9125TZKXH"></a>

### Keep vocabulary out of the fact bridge

| Type | Decision |
| --- | --- |
| ID | ADR-GENERIC-FACT-BRIDGE |
| Status | Accepted |
| Justifies | [DES-FACT-BRIDGE](index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) |
| Justifies | [DES-REQUEST-INPUTS](index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG) |

- Chosen: `mara.dl` declares only what every vocabulary has. A template declares its own inputs, such as results, and a request supplies them.
- Rejected: `verification_result` and `validation_result` in `mara.dl`. Other vocabularies would carry them, and changing a column would need a Mara release.
- Rejected: stored result files. Run results are not documentation.

---

<a id="01M7CT60PP6JY095YYYJMT9TVV"></a>

### Let Mara compute a gate's outcome

| Type | Decision |
| --- | --- |
| ID | ADR-MARA-COMPUTES-GATE-OUTCOME |
| Status | Accepted |
| Justifies | [DES-GATE-OUTCOME](index.mara.md#01M7CT60KEEZRJCPJ0CPT7YT5B) |

- Chosen: a gate names subject, condition and unavailable relations, and Mara derives the outcome of each named item.
- Rejected: an outcome relation that each program fills. Every program would repeat the precedence of failed over unavailable and the handling of a wrong or empty scope.
- Rejected: a separate `required_validation` gate, as in the previous template. A validation is a named item of `release-ready`.

---

<a id="01M7CZA3GYB8DH1YPKH0T06JZP"></a>

### Generate every input and output directive, as JSON

| Type | Decision |
| --- | --- |
| ID | ADR-GENERATED-JSON-IO |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |
| Justifies | [DES-FACT-BRIDGE](index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J) |

- Chosen: programs hold no input or output directive. Mara generates them for the relations a run needs and uses Soufflé's JSON files; a run at 16× takes 18 ms instead of 14 ms ([EVD-SOUFFLE-TRIAL](evaluation.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN)).
- Rejected: Soufflé's default tab-separated files. A tab or a newline in a value reads as a column or row break ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).
- Rejected: escaping values in tab-separated files. A program can still build such a value, and rules would see the escaped text.

---

<a id="01M7K0ZGQTPCKA5Z5M49NW7C35"></a>

### Stop an evaluation at a limit without a crash record

| Type | Decision |
| --- | --- |
| ID | ADR-LIMIT-STOPS-NOT-CRASHES |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |

- Chosen: `RLIMIT_AS` and `RLIMIT_FSIZE` keep their ends, an abort and `SIGXFSZ`. The child's soft core limit is exactly 1 byte where the inherited hard limit allows it; the host never changes the hard limit, so the core limit never fails an evaluation.
- Where `core_pattern` pipes to a handler such as systemd-coredump, the kernel starts no handler at a core limit of 1 byte, so neither stop is recorded as a crash (in mainline since 2.6.35; measured on Linux 7.2, mara-2dnzjc). At 0 the handler still records the crash without its core. A pattern that names a file writes nothing at 1 byte.
- A record remains where `core_pattern` names a socket (`@@…`, systemd's socket handler), which the kernel starts at any core limit, and with a piped handler where the inherited hard core limit is 0. mara-8dtsks holds these cases.
- Rejected: a cgroup `memory.max` through `systemd-run --user --scope` for each evaluation. It needs a systemd user manager and its bus, adds variables to Soufflé's empty environment and a process start to each evaluation, and tests would use a daemon they did not start.
- Rejected: a cgroup that Mara creates itself. Its memory controller needs a parent cgroup without processes, which an unprivileged user has only when systemd or an administrator delegates one; Mara cannot rely on either.
- Rejected: Mara polling the child's memory and killing it before the limit. An allocation that jumps past the limit between two polls still aborts.
- Rejected: a Soufflé patched to exit on `std::bad_alloc` without aborting. It changes the pinned build, and `abort()` cannot be ignored or blocked from outside.
- Rejected: `SIGXFSZ` ignored and the output limit read from file sizes. The core limit already covers a piped handler and a file pattern; it is one way to cover the output limit under a socket handler (mara-8dtsks).

---

<a id="01M7K4XZPESXME0JGJMP2CAXPS"></a>

### Contain Soufflé with Landlock and seccomp on Linux

| Type | Decision |
| --- | --- |
| ID | ADR-SOUFFLE-SANDBOX |
| Status | Accepted |
| Justifies | [DES-SOUFFLE-SANDBOX](evaluation.mara.md#01M7K4XGJ4TSYM1E64C327HXJQ) |

- Chosen by the maintainer on 2026-11-16: an operating-system sandbox around the `souffle` child process on Linux, built before 0.4.0; macOS follows with the macOS work. Programs come with checkouts the user did not write, five reviews found ways past the admission check, and a memory fault in Soufflé is outside any check ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).
- Rejected by the maintainer: a Decision that accepts [RISK-SOUFFLE-REACHES-OUTSIDE](index.mara.md#01M7CYC90BZBBXGC0CZ66W8ZEV); a sandbox after 0.4.0.
- Chosen: Landlock for files, programs, sockets and signals, and a seccomp filter for what Landlock leaves open ([DES-SOUFFLE-SANDBOX](evaluation.mara.md#01M7K4XGJ4TSYM1E64C327HXJQ)).
- Rejected: Landlock alone. On every ABI it leaves changes of mode, owner, times and attributes by path, and the keyrings; below ABI 6 signals to other processes, below ABI 9 and 10 UNIX and UDP sockets. Ubuntu 24.04's kernel 6.8 has ABI 4.
- Rejected: seccomp alone. It cannot tell paths apart.
- Rejected: user namespaces through `bwrap` or `unshare`. Ubuntu 24.04 restricts unprivileged user namespaces, and they add a program to install and a process to each evaluation.
- Chosen: the kernel interface through `libc`, already a dependency. The parent builds the ruleset and the filter; the child makes four system calls, which are async-signal-safe and allocate nothing. Rejected: the `landlock` crate. It allocates, so it too could only run in the parent, and it adds dependencies for about 60 lines.
- Chosen: reading files beneath the interpreter's directory, rather than only the libraries the binary needs. The loader may load a variant from a subdirectory (`glibc-hwcaps`), and finding the exact files means repeating its search. That directory holds system libraries, no user data.
- Chosen on 2026-11-20, after review: only that directory, because Mara runs only the Soufflé it ships or builds, whose libraries are all there. Rejected for now: allowing the directories that the loader resolves for a Soufflé with libraries elsewhere; Mara runs none (backlog mara-kga2ga).
- Chosen: marking every inherited descriptor close-on-exec with `close_range`. Rejected: closing them, which would break how Rust's `Command` reports a failed `exec`; a loop over descriptor numbers, up to a million calls.
- Chosen: allowing one `ioctl` request, `TCGETS`, rather than refusing listed ones. Requests that work on any file and change it or direct a signal are many and grow.

---

<a id="01M7K4XZRS4DDAREPZSWC0WZF7"></a>

### Evaluate no program without the sandbox

| Type | Decision |
| --- | --- |
| ID | ADR-SANDBOX-REQUIRED |
| Status | Accepted |
| Justifies | [DES-SOUFFLE-SANDBOX](evaluation.mara.md#01M7K4XGJ4TSYM1E64C327HXJQ) |

- Chosen by the worker of mara-ptq6xc on 2026-11-16, the stricter reading of [REQ-PROGRAMS-ONLY-COMPUTE](index.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86), and confirmed by the maintainer on 2026-11-20: on Linux without Landlock, or wherever the sandbox cannot be built or entered, Mara evaluates no program. Each evaluation is a fault that names Landlock, or the step that failed, and how to fix it, and Soufflé never starts.
- Landlock ABI 1 suffices: the seccomp filter closes what it leaves open ([DES-SOUFFLE-SANDBOX](evaluation.mara.md#01M7K4XGJ4TSYM1E64C327HXJQ)). Landlock is on in the default kernels of current Ubuntu, Debian, Fedora and Arch Linux, per their kernel configurations; measured only on Arch Linux with Linux 7.2, ABI 10. Read from the published profiles and kernel configurations, not run: Docker's default seccomp profile refuses Landlock's system calls up to Docker 20.10.7, gVisor does not offer Landlock, and the WSL 2 kernel does.
- Rejected: evaluating without the sandbox, with or without a warning. A checkout could then reach outside exactly on the hosts that lack it, and a warning undoes no read or write.
- Rejected: requiring ABI 6, 9 or 10, which cover signals, UNIX sockets and UDP themselves. They refuse Ubuntu 24.04's kernel 6.8 (ABI 4), and the filter covers those.
- Rejected: an option that allows evaluation without the sandbox. It adds a capability that loosens a security requirement, which only the maintainer decides.

---

<a id="01M89DA9KA577J1HF2SMKHABSH"></a>

### Answer a browser request that evaluates programs by passes

| Type | Decision |
| --- | --- |
| ID | ADR-BROWSER-EVALUATION-PASSES |
| Status | Accepted |
| Justifies | [DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) |

- Chosen during the rewrite on 2026-11-25 (mara-xsm8w6): Mara's module answers the runs of Soufflé that a request waits for, and runs the request again once the workspace's worker supplied what they left, until a pass waits for none. Planning, the private directory's files, the reading of what Soufflé printed and wrote, faults and the explanation console are the native host's code; only the start of Soufflé differs. The cost is one more pass of the request per wave of runs, on inputs the workspace keeps, and for an explanation one run of Soufflé per console command.
- Rejected: suspending the module during a run with JavaScript Promise Integration or Asyncify. JSPI is missing from browsers the package serves, and Asyncify rewrites Mara's module, which grows and slows it.
- Rejected: a synchronous wait on Soufflé's worker through `SharedArrayBuffer` and `Atomics.wait`. It needs a cross-origin isolated page.
- Rejected: asynchronous operations in the library, Rust `async` from each evaluation up to every operation. It changes every native call site for one host.
- Rejected: the explanation console in Soufflé's worker, through a second instance of Mara's module. The rows to explain come from the request's snapshot, which only the workspace's worker holds, so the console's state and the chosen rows would cross the module's boundary.
