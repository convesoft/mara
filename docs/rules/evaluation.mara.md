# Rule evaluation

## Requirements

---

<a id="01M7BF6HCQX66B3MYRH5BZM6BE"></a>

### Explain an outcome down to document locations

| Type | Requirement |
| --- | --- |
| ID | REQ-EXPLAIN-OUTCOMES |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](../product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

For a row of an invariant or gate outcome, Mara returns an explanation whose leaves are input facts with document path and line, or, for a fact that a supplied input brought, the name of that input, or reports that Soufflé cannot explain that program. The outcome stands either way.

---

<a id="01M7BF6HCZ88PSSQTFKXE8KJ81"></a>

### A faulty program never passes

| Type | Requirement |
| --- | --- |
| ID | REQ-PROGRAM-FAULTS-REPORTED |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](../product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

A program that fails to compile is reported with file, line and caret. A compile error or a runtime fault makes the outcome unavailable, never passed.

---

<a id="01M7CMW70G711Z67QEWW5Z0MK0"></a>

### An evaluation that runs too long is stopped

| Type | Requirement |
| --- | --- |
| ID | REQ-EVALUATION-TIME-LIMIT |
| Status | Draft |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](../product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

An evaluation still running at its time limit is stopped, and its outcome is unavailable, never passed. The limit is 10 s unless the configuration sets another; the number is provisional.

---

## Design

---

<a id="01M7BF6HD6K4B2K5Q7FJVGCDNH"></a>

### Running Soufflé

| Type | Design |
| --- | --- |
| ID | DES-RULE-EXECUTION |
| Status | Accepted |
| Satisfies | [REQ-EXPLAIN-OUTCOMES](#01M7BF6HCQX66B3MYRH5BZM6BE) |
| Satisfies | [REQ-PROGRAM-FAULTS-REPORTED](#01M7BF6HCZ88PSSQTFKXE8KJ81) |

- An evaluation is keyed by the keys of the layers it reads (the document snapshot, and the code layer when the program uses code links), the hash of the program with every file it includes, and the inputs the request supplied with their completeness marks ([DES-INPUTS-AND-KEYS](../engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY), [DES-REQUEST-INPUTS](index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).
- It runs only programs that Mara's parser admits, on the parsed copies in a private directory ([DES-PROGRAM-PROFILE](index.mara.md#01M7CMW7051QVPFDWQBMB1A6K6)): a new directory under the system's temporary directory, removed when the evaluation ends.
- A declared program is a file that a `[rules] programs` pattern matches ([DES-RULE-CONSUMERS](index.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)). `project validate` runs each one that an entry names or that no other declared file includes, then, in path order, each that no run so far reads, such as a file of an include cycle; each once, with the snapshot's facts when an invariant uses it, else against empty facts (2–6 ms), which compiles it. So every declared file is read by a run or refused. `item validate` runs only the programs that invariants use, every entry of such a program shaping the run, and reports their faults with the item's diagnostics. There is no compile-only operation.
- Validation supplies no input: each input that an entry of the program lists is read as an empty relation, and `source_complete` holds no row.
- The private copies are `mara.dl` for Mara's declarations and `p<n>.dl` for the program's other files, `n` counting them in the order the parser read them, the declared program first. In each copy, every `.include` path is replaced by the name of the copy that the parser resolved it to, and nothing else changes. So Soufflé reads exactly the files the parser read, and its diagnostics, which name a file only by its base name (`in file q.dl` for `sub/q.dl`), name one file each.
- For each run the host generates a wrapper program in the private directory. It includes the consumer's program and holds one `.input` directive for each fact relation and supplied input and one `.output` directive for each relation that a configuration entry names in that program, all as JSON files in that directory (`IO=jsonfile`, `format=list`), the outputs in its subdirectory `out/`. JSON keeps tabs, newlines and quotes in values, which Soufflé's default tab-separated files do not.
- Native: `souffle --no-preprocessor -l "" <wrapper>.dl` as a child process, single-threaded, with the private directory as its working directory, an empty environment, no standard input, and its standard output and error written to files there. `-l ""` stops Soufflé from loading a `libfunctors.so`, which it otherwise does for every program. On Linux it runs in its sandbox ([DES-SOUFFLE-SANDBOX](#01M7K4XGJ4TSYM1E64C327HXJQ)). The host finds the binary as [DES-SOUFFLE-BUILD](../distribution.mara.md#01M7BF6GX9WNFK70C6H1RBQJ5R) says.
- Natively, resource limits of the child process bound an evaluation besides its time: 1 GiB of address space (`RLIMIT_AS`) and 64 MiB per file it writes (`RLIMIT_FSIZE`), an output relation or what it prints. Past the memory limit Soufflé fails with `std::bad_alloc` and aborts; past the file size the system ends it with `SIGXFSZ`. Its soft core limit is 1 byte (`RLIMIT_CORE`) where the inherited hard limit allows it, so neither stop writes a core or is recorded as a crash where `core_pattern` pipes to a handler such as systemd-coredump or names a file; Linux logs one kernel line (`RLIMIT_CORE is set to 1, aborting core`) instead ([ADR-LIMIT-STOPS-NOT-CRASHES](decisions.mara.md#01M7K0ZGQTPCKA5Z5M49NW7C35)). On a host whose `core_pattern` names a socket, or pipes to a handler while the hard core limit for `mara` is 0, the user still sees each such stop as a crash of `souffle` in the crash records, without a core, beside the fault that names the limit. The host reads at most 64 KiB of what it printed and quotes at most 4 KiB in a diagnostic.
- An evaluation never outlives `mara`: on Linux the child gets SIGKILL when the thread that started it ends (`PR_SET_PDEATHSIG`). SIGINT, SIGTERM or SIGHUP during an evaluation stops the child and removes the private directory, then ends `mara` by the signal's default action; at other times they act as by default. After SIGKILL the private directory stays under the temporary directory until the system cleans it; nothing reads it again.
- Browser: Soufflé runs in a worker of its own, apart from the one that holds the workspace. Compile `souffle.wasm` once per such worker. For each evaluation, create a fresh instance, write programs and facts into its in-memory filesystem and read the outputs back. The module is built with `-sEXIT_RUNTIME=1` or exports `callMain`, so the host receives the exit code of `main`.
- Every consumer of a program reads the same run: the outputs do not depend on which consumer asked.
- Numbers are signed 32-bit integers on every host. A row in an output file that does not fit the relation's columns, or a value that is not valid UTF-8 (`substr` counts bytes), is a fault.
- Explanations run on demand in provenance mode (`-t explain`), driven on stdin by `format json`, then `explain <relation>(<args>)`. Use JSON: the text proof format garbles negated atoms. In the browser the host supplies that input through the module's `stdin` hook. When provenance mode aborts, the host reports the explanation as unavailable.
- The `explain` command does not decode escapes, and a default-depth proof holds unresolved `subproof` references. The host expands them, checks that the proof it gets back is for the row it asked about, and reports the explanation as unavailable when the row's values cannot be written in the command (quotes, backslashes, control characters, negative numbers), when the reply is not valid JSON, when a branch ends in an aggregate's internal relation, or when the exchange passes the time limit. It leaves constraint leaves out: Soufflé prints internal numbers for symbols there. A leaf from a supplied input names that input and holds the row's values as supplied, instead of a location.
- The run in provenance mode is the request's one run: `souffle -t explain` started as above, its standard input a pipe that a thread of the host writes, its standard output a pipe that another thread reads, at most 16 MiB, so that neither can hold the run past its time limit. When the system cannot start either thread, the host kills Soufflé and the evaluation is a fault of the host. The host first sends `format json` and `explain __mara_ready()`, a name no admitted program declares. Soufflé answers only once the evaluation ended and wrote its outputs; at that reply the host interprets what Soufflé printed on its standard error so far, as for any run, reads the outputs, and only then chooses the rows to explain and sends one command at a time. A fault before that reply is the evaluation's fault and leaves the outcome unavailable. After it, the time limit, an abort, a stray or invalid reply, or more than 16 MiB of replies leave the outcome as the outputs gave it and each row not yet explained without an explanation, naming why; a row explained before keeps its proof.
- A leaf of `item` or `edge` is at the path and line its row holds; one of `code_link` at its code file and the marker's line, at the grammar level too, where its values hold the owning declaration's line and name ([DES-FACT-BRIDGE](index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)); one of `title`, `revision` or `field` at its item's; one of `folded` at the first item, in snapshot order, whose title or field value is its text. A place found through a MID that several items have, as after an item was copied with its anchor, is no one place: such a leaf cannot be explained, and the reason names the MID. A leaf of a relation the program states by facts, such as the `engineering` template's `relation_class`, is at the program file and line of the fact that writes that row with literal values; a row that no such fact writes, or of a relation with an aggregate in one of its rules or that the program does not declare, cannot be explained. A leaf matches a row when Soufflé prints that row as the leaf; a leaf that matches no row, or rows with different values, cannot be explained.
- Soufflé prints a rule's string constants between quotes as they are, without escapes, decoded as Mara's parser decodes them. The host reads a printed rule with the program's own string constants: at each quote, the one constant that a quote closes there; when none or several do, the rule cannot be read and the row cannot be explained. A rule is shown on one line by making each run of whitespace outside its constants one space.
- A proof passes an aggregate when a rule of one of its steps holds one (`: {` outside its constants), even when its branch ends in an input. A step's children are its atoms and negations in the order of its rule, then its constraints; so no atom or negation follows a constraint, their number is the rule's, and the nth negation is the rule's nth negated literal; a reply that breaks this is not a proof. A negated literal with a wildcard is shown as the rule writes it, since Soufflé prints arbitrary values for wildcards, and one without as Soufflé prints it. An expanded `subproof` must be a step of the relation its reference names. A proof of more than 1,000 steps, or of more than 32 KiB in a result, cannot be explained.
- A constraint leaf is one of the forms Soufflé 2.5 prints for a binary constraint (its `constraintList`): `<number> <operator> <number>` for `=`, `!=`, `<`, `<=`, `>=`, `>`, a symbol as its internal number; and `<name>("<symbol>", "<symbol>")` for `match`, `contains`, `not_match` and `not_contains`, the last two being how it prints a negated `match` or `contains`. Where the program declares a relation named `not_match` or `not_contains`, such a leaf cannot be told from a row of it and the row cannot be explained.
- A runtime fault such as `to_number` of text that is not a number ends the child process with the rule's location natively, and aborts the module without a location in WASM.
- Soufflé's compile diagnostics name a file and a line. Anything else it prints on stderr or stdout during a run, such as the warning for a `substr` out of range, is a runtime fault, also when it exits with 0. A compile warning, such as for a relation without rules or facts, is a compile error for Mara: only an invariant gives a warning ([DES-DIAGNOSTICS](../validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)).
- A compile error, a refusal or a fault is an error diagnostic at the project path of the file it concerns, never the private copy's, and at its line; a compile error's message ends with the source line and a caret under the column. A fault without a line, or from a limit, which it names, is at the configuration line that declares the program. `project validate` is then invalid, `query execute` and `query view` fail, and a gate and every item it names are unavailable. The codes are `program-refused` for a refusal, `program-error` for a compile diagnostic and `program-fault` for the rest.
- At the time limit ([REQ-EVALUATION-TIME-LIMIT](#01M7CMW70G711Z67QEWW5Z0MK0)) the host kills the child process natively and terminates the Soufflé worker in the browser; the workspace and its unexported edits are not in that worker.

---

<a id="01M7K4XGJ4TSYM1E64C327HXJQ"></a>

### Soufflé's sandbox on Linux

| Type | Design |
| --- | --- |
| ID | DES-SOUFFLE-SANDBOX |
| Status | Accepted |
| Satisfies | [REQ-PROGRAMS-ONLY-COMPUTE](index.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |
| Mitigates | [RISK-SOUFFLE-REACHES-OUTSIDE](index.mara.md#01M7CYC90BZBBXGC0CZ66W8ZEV) |

On Linux the `souffle` child process enters a sandbox between fork and exec, after its limits ([DES-RULE-EXECUTION](#01M7BF6HD6K4B2K5Q7FJVGCDNH)): every descriptor from 3 up marked close-on-exec (`close_range`, Linux 5.11), no-new-privileges, then a Landlock domain, then a seccomp filter. So at `exec` it holds only its standard input, output and error, whichever descriptors `mara` inherited. The host builds the ruleset and the filter for each evaluation before it starts the child.

| Object | Allowed |
| --- | --- |
| The private directory | Read files, list directories |
| Its `out/` | Also create, write and truncate regular files |
| The Soufflé binary | Execute, read |
| The ELF interpreter that the binary names (`PT_INTERP`) | Execute, read |
| The directory that holds the interpreter, after symbolic links | Read files: the shared libraries |
| `/etc/ld.so.cache` | Read |

- The ruleset handles every access right that the kernel's Landlock ABI knows: files, TCP and UDP, pathname and abstract UNIX sockets, signals. Each right that the table does not allow is denied.
- The filter fails these system calls with `EPERM`, which Landlock leaves open on every ABI or on older ones:
  - `socket` and `socketpair`, and `io_uring_setup`, which opens sockets past the filter; no other call yields a socket without an inherited one;
  - `kill`, `tkill`, `tgkill`, `rt_sigqueueinfo` and `rt_tgsigqueueinfo` unless their first argument is the child's process ID; `pidfd_send_signal`; `fcntl` with `F_SETOWN`, `F_SETOWN_EX` or `F_SETSIG`, which direct the signal of asynchronous I/O to a chosen process;
  - every `ioctl` but `TCGETS`, by which the C library asks whether a stream is a terminal: among them `FIOSETOWN`, `SIOCSPGRP` and `FIOASYNC`, and `FS_IOC_SETFLAGS` and `FS_IOC_FSSETXATTR`, which change a file's attribute flags;
  - the changes of a file's mode, owner, times or extended attributes, by path or through a descriptor (`chmod`, `fchmod`, `fchmodat`, `fchmodat2`, `chown`, `lchown`, `fchown`, `fchownat`, `utime`, `utimes`, `futimesat`, `utimensat`, the `setxattr` and `removexattr` families); `truncate`;
  - `keyctl`, `add_key` and `request_key`; every system call of another architecture or of the x32 ABI.
- So the child is contained alike on Landlock ABI 1 (Linux 5.13) and later. Without Landlock, or on an architecture without a filter (Mara has them for x86-64 and arm64), the evaluation does not start ([ADR-SANDBOX-REQUIRED](decisions.mara.md#01M7K4XZRS4DDAREPZSWC0WZF7)): its fault says that Mara runs Soufflé only in a Landlock sandbox and how to enable Landlock.
- The loader's directory holds every library of the Soufflé that Mara runs: the one it ships or that `scripts/build-souffle.sh` builds, linked only against the C library, `libm` and the loader, which glibc installs together ([DES-SOUFFLE-BUILD](../distribution.mara.md#01M7BF6GX9WNFK70C6H1RBQJ5R)); Mara never runs another. A Soufflé whose libraries lie elsewhere (RPATH, `/opt`, Nix with nix-ld, Homebrew on Linux) cannot load them: its fault names the library and this rule.
- A step of entering the sandbox that fails stops the child before `exec`, so Soufflé is not started; that includes a kernel or an outer filter that refuses `close_range`. The fault names the step and says that a confinement Mara runs in, such as a container's seccomp profile or an agent's sandbox, must permit `close_range`, no-new-privileges, Landlock and seccomp filters.
- The child can still read file metadata outside (`stat`, `access`), which Landlock does not restrict, and it can run code it maps from memory, inside the same sandbox.
- A denied call fails with an error. Soufflé reports a file it cannot read, so the evaluation is a fault; it skips an output file it cannot open without a message ([EVD-SOUFFLE-PROGRAM-REACH](#01M7CMW70TAFCJKPAX3VY94QMG)), so such a write is stopped and the outcome holds the program's rows.
- On macOS Soufflé runs without a sandbox.

---

## Evidence

---

<a id="01M7BF6HDDJJ0Y0WWT6RK17CDN"></a>

### Soufflé trial

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-TRIAL |
| Status | Accepted |
| Supports | [ADR-SOUFFLE-DATALOG](decisions.mara.md#01M7BF6GSZ16MZQAGBY5M9TDKZ) |
| Supports | [ADR-SOUFFLE-AS-PROCESS](decisions.mara.md#01M7BF6GT9PVXSKV1ANGEWA7WR) |

Soufflé 2.5, 2026-11-13. Corpora: 1× = 200 items and 266 edges, 16× = 3,200 and 4,256, 64× = 12,800 and 17,024. Times include process start and program compilation.

| Check | Result |
| --- | --- |
| Risk score with a threshold parameter; missing inputs reported as unavailable | Pass |
| Coverage: at least one accepted verification | Pass |
| Ancestry across a cycle; cycle members found | Pass |
| Missing versus unavailable external results, by completeness | Pass |
| JSON explanation of a failed gate condition, with document locations | Pass |
| `implementation_ready` gate: 5 conditions plus an unavailable outcome | Pass |
| Compile errors: undefined relation, arity, `Mid` versus `Id`, symbol versus number, unstratifiable negation | All caught, with file, line and caret |
| Native gate time: empty, 1×, 16×, 64× | 6.1, 6.0, 11.3, 29.5 ms; 18 MB peak memory at 64× |
| Native growth from 16× to 64× (4× data) | Time ×2.6–2.8, instructions ×2.9–3.5 |
| `-j8` against `-j1` | Slower |
| Provenance mode overhead at 16× | 1.5× (21.0 against 14.2 ms) |
| WASM gate time: empty, 1×, 16×, 64× | 11.9, 13.4, 21.7, 44.8 ms |
| Native against WASM outputs, all programs at 16× | Identical (6,696 rows) |
| JSON facts and outputs through generated directives, 2026-11-14 | Tabs, newlines, quotes and backslashes in values round-trip exactly; a component's relation is written; trial gate at 16×: 18 ms against 14 ms tab-separated, same 960 rows |
| WASM explanation, 2026-11-14: console input from the module's `stdin` hook, under Node 26 | Two proofs, one through a negated atom, byte-identical to native; 110 ms at 16× |
| Native binary, static C++ runtime, no OpenMP | 15 MB stripped, 2.6 MB xz; needs only libc and libm |
| WASM module | 6.1 MB, 1.2 MB gzip, 0.7 MB Brotli |
| Build time, 10 jobs | Native 107–120 s; Emscripten 66 s |
| Trial gate compiled with `souffle -g` | 3,261 lines of C++; `g++ -O2` 10.3 s, `clang++ -O2` 6.4 s, `g++ -O0` 4.6 s |

The trial gates approximate the gates of the `engineering` template at commit `d7f0e73`. Artifacts (Soufflé source and builds, programs, fact generator, WASM runner, benchmarks) are on the maintainer's machine in `~/Work/local/workspace/scratch/souffle-spike/`.

---

<a id="01M7CMW70TAFCJKPAX3VY94QMG"></a>

### Soufflé programs reach outside their evaluation

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-PROGRAM-REACH |
| Status | Accepted |
| Supports | [ADR-PROGRAM-PROFILE](decisions.mara.md#01M7CMW6ZGC0PY8DVYNQDSBPRH) |
| Supports | [RISK-SOUFFLE-REACHES-OUTSIDE](index.mara.md#01M7CYC90BZBBXGC0CZ66W8ZEV) |

The static binary of [EVD-SOUFFLE-TRIAL](#01M7BF6HDDJJ0Y0WWT6RK17CDN), run on 2026-11-14 as `souffle --no-preprocessor -F <facts> -D <out> <program>.dl`, from the fourth row on with `-l ""` where stated.

| Program | Result |
| --- | --- |
| `.output r(IO=file, filename="<absolute path>")` | Wrote that file, outside `<out>` |
| `.input s(IO=file, filename="/etc/hostname")` | Read that file into `s` |
| `.functor f(x: number): number`, with a `libfunctors.so` in the working directory | Loaded the library and ran its code |
| `.pragma "library-dir"` and `.pragma "libraries"` naming a library elsewhere | Loaded that library and ran its code |
| `.pragma "output-dir"` | No effect; `-D` won |
| Any program, with a `libfunctors.so` in the working directory or on `LD_LIBRARY_PATH` | Loaded the library and ran its constructor, with no `.functor` in the program. `-l ""` prevented it |
| `#line 1 "<file>"` or `# 1 "<file>"` before a faulty declaration | The diagnostic printed the first line of that file, with `--no-preprocessor` |
| `#line 1 "<file>"` after other text on the same line | The same file read |
| `__FILE__` in a fact | The absolute path of the program in its private directory, different in each directory |
| Default tab-separated output of one row whose value holds a newline and a tab | The same bytes as two separate rows |
| `.decl r(x: symbol) output printsize`, under a wrapper without output directives | Wrote `r.csv` in the working directory |
| `choice-domain`, with the wrapper writing one more relation | A different answer for the same input: `("a","c")` became `("a","b")` |
| JSON input of 1.5 into a `float` column, and of `"not-a-number"` into a `number` column | 1.4013e-45 and 0, without an error |
| `2 ^ 40` | −2147483648 natively, 2147483647 in WASM |
| `+`, `*`, shifts, `/` and `%` at the limits of 32 bits, `strlen`, `to_number` of a hexadecimal string | The same on both hosts; division by zero and an unparsable `to_number` fail on both |
| `match("[", m)` in a rule | `warning: wrong pattern provided`, exit 0, no rows |
| `explain` for a row whose value holds a newline, written as an escape | A proof for another row, whose value holds a backslash and `n` |
| `r(as(n, symbol)) :- a(n).` with `a(0)` | Segmentation fault natively; a row with an empty symbol in WASM |
| `.decl a(x: number) magic` with a subsumption clause, read by a gate condition | The clause is dropped: the condition has no row, on both hosts; with `no_magic` it has one |
| `match("\\cI", "\t")` | No row natively, one row in WASM, no message on either |
| `x % y` with −2147483648 and −1 | Arithmetic exception natively; 0 in WASM, so a condition on it passes |
| `x / 0` | Arithmetic exception on x64 and a trap in WASM. On ARM64 the division instruction does not trap and yields 0: read from Soufflé's code and the compiled instruction, not run |
| `to_number(2147483648.0)`, a float literal without the word `float` | −2147483648 natively, 2147483647 in WASM |
| `explain` of `r(x) :- a(x), x < "z".` | A leaf `4 < 5`: internal numbers instead of the symbols |
| Upstream on 2026-11-14 | 2.5 is the latest release. Fixed on master only: `magic` with subsumption (issue 2322) and an explanation fix (2585). By design: `choice-domain` order, the `libfunctors.so` load, `#line`, unchecked `as`. Unchanged on master: the JSON reader |
| `-t explain` on a rule with `autoinc()`, on a rule with an ADT constructor in its body, or on a program with a subsumption clause | Aborted; normal evaluation of the same programs succeeded |
| `-t explain` with records, aggregates, negation, components or `eqrel` | Explained |
| `.output r(IO=file, filename="<path>")` and `IO=jsonfile`, where it cannot open the path; run on 2026-11-16 | Exit status 0, no message, no file |
| `nat(0). nat(x + 1) :- nat(x).` | Still running when killed after 5 s |
| `s("a"). s(cat(x, "a")) :- s(x).` | Still running when killed after 5 s |

The two programs that never ended use no records, ADTs or `autoinc()`.
