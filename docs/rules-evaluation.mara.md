# Rule evaluation

## Requirements

---

<a id="01M7BF6HCQX66B3MYRH5BZM6BE"></a>

### Explain an outcome down to document locations

| Type | Requirement |
| --- | --- |
| ID | REQ-EXPLAIN-OUTCOMES |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

For a row of an invariant or gate outcome, Mara returns an explanation whose leaves are input facts with document path and line, or, for a fact that a supplied input brought, the name of that input, or reports that Soufflé cannot explain that program. The outcome stands either way.

---

<a id="01M7BF6HCZ88PSSQTFKXE8KJ81"></a>

### A faulty program never passes

| Type | Requirement |
| --- | --- |
| ID | REQ-PROGRAM-FAULTS-REPORTED |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

A program that fails to compile is reported with file, line and caret. A compile error or a runtime fault makes the outcome unavailable, never passed.

---

<a id="01M7CMW70G711Z67QEWW5Z0MK0"></a>

### An evaluation that runs too long is stopped

| Type | Requirement |
| --- | --- |
| ID | REQ-EVALUATION-TIME-LIMIT |
| Status | Draft |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

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

- An evaluation is keyed by the keys of the layers it reads (the document snapshot, and the code layer when the program uses code links), the hash of the program with every file it includes, and the inputs the request supplied with their completeness marks ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY), [DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).
- It runs only programs that Mara's parser admits, on the parsed copies in a private directory ([DES-PROGRAM-PROFILE](rules-facts.mara.md#01M7CMW7051QVPFDWQBMB1A6K6)): a new directory under the system's temporary directory, removed when the evaluation ends.
- A declared program is a file that a `[rules] programs` pattern matches ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)). `project validate` runs each one that an entry names or that no other declared file includes, then, in path order, each that no run so far reads, such as a file of an include cycle; each once, with the snapshot's facts when an invariant uses it, else against empty facts (2–6 ms), which compiles it. So every declared file is read by a run or refused. `item validate` runs only the programs that invariants use, every entry of such a program shaping the run, and reports their faults with the item's diagnostics. There is no compile-only operation.
- Validation supplies no input: each input that an entry of the program lists is read as an empty relation, and `source_complete` holds no row.
- The private copies are `mara.dl` for Mara's declarations and `p<n>.dl` for the program's other files, `n` counting them in the order the parser read them, the declared program first. In each copy, every `.include` path is replaced by the name of the copy that the parser resolved it to, and nothing else changes. So Soufflé reads exactly the files the parser read, and its diagnostics, which name a file only by its base name (`in file q.dl` for `sub/q.dl`), name one file each.
- For each run the host generates a wrapper program in the private directory. It includes the consumer's program and holds one `.input` directive for each fact relation and supplied input and one `.output` directive for each relation that a configuration entry names in that program, all as JSON files in that directory (`IO=jsonfile`, `format=list`), the outputs in its subdirectory `out/`. JSON keeps tabs, newlines and quotes in values, which Soufflé's default tab-separated files do not.
- Native: `souffle --no-preprocessor -l "" <wrapper>.dl` as a child process, single-threaded, with the private directory as its working directory, an empty environment, no standard input, and its standard output and error written to files there. `-l ""` stops Soufflé from loading a `libfunctors.so`, which it otherwise does for every program. The host finds the binary as [DES-SOUFFLE-BUILD](distribution.mara.md#01M7BF6GX9WNFK70C6H1RBQJ5R) says.
- Natively, resource limits of the child process bound an evaluation besides its time: 1 GiB of address space (`RLIMIT_AS`) and 64 MiB per file it writes (`RLIMIT_FSIZE`), an output relation or what it prints. Past the memory limit Soufflé fails with `std::bad_alloc`; past the file size the system ends it with `SIGXFSZ`. It writes no core file. The host reads at most 64 KiB of what it printed and quotes at most 4 KiB in a diagnostic.
- An evaluation never outlives `mara`: on Linux the child gets SIGKILL when the thread that started it ends (`PR_SET_PDEATHSIG`). SIGINT, SIGTERM or SIGHUP during an evaluation stops the child and removes the private directory, then ends `mara` by the signal's default action; at other times they act as by default. After SIGKILL the private directory stays under the temporary directory until the system cleans it; nothing reads it again.
- Browser: Soufflé runs in a worker of its own, apart from the one that holds the workspace. Compile `souffle.wasm` once per such worker. For each evaluation, create a fresh instance, write programs and facts into its in-memory filesystem and read the outputs back. The module is built with `-sEXIT_RUNTIME=1` or exports `callMain`, so the host receives the exit code of `main`.
- Every consumer of a program reads the same run: the outputs do not depend on which consumer asked.
- Numbers are signed 32-bit integers on every host. A row in an output file that does not fit the relation's columns, or a value that is not valid UTF-8 (`substr` counts bytes), is a fault.
- Explanations run on demand in provenance mode (`-t explain`), driven on stdin by `format json`, then `explain <relation>(<args>)`. Use JSON: the text proof format garbles negated atoms. In the browser the host supplies that input through the module's `stdin` hook. When provenance mode aborts, the host reports the explanation as unavailable.
- The `explain` command does not decode escapes, and a default-depth proof holds unresolved `subproof` references. The host expands them, checks that the proof it gets back is for the row it asked about, and reports the explanation as unavailable when the row's values cannot be written in the command (quotes, backslashes, control characters, negative numbers), when the reply is not valid JSON, when a branch ends in an aggregate's internal relation, or when the exchange passes the time limit. It leaves constraint leaves out: Soufflé prints internal numbers for symbols there. A leaf from a supplied input names that input and holds the row's values as supplied, instead of a location.
- A runtime fault such as `to_number` of text that is not a number ends the child process with the rule's location natively, and aborts the module without a location in WASM.
- Soufflé's compile diagnostics name a file and a line. Anything else it prints on stderr or stdout during a run, such as the warning for a `substr` out of range, is a runtime fault, also when it exits with 0. A compile warning, such as for a relation without rules or facts, is a compile error for Mara: only an invariant gives a warning ([DES-DIAGNOSTICS](validation.mara.md#01M7CMW71GXWZS8HPVVTYPENDK)).
- A compile error, a refusal or a fault is an error diagnostic at the project path of the file it concerns, never the private copy's, and at its line; a compile error's message ends with the source line and a caret under the column. A fault without a line, or from a limit, which it names, is at the configuration line that declares the program. `project validate` is then invalid, `query execute` and `query view` fail, and a gate and every item it names are unavailable. The codes are `program-refused` for a refusal, `program-error` for a compile diagnostic and `program-fault` for the rest.
- At the time limit ([REQ-EVALUATION-TIME-LIMIT](#01M7CMW70G711Z67QEWW5Z0MK0)) the host kills the child process natively and terminates the Soufflé worker in the browser; the workspace and its unexported edits are not in that worker.
