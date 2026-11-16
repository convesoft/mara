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
- `project validate` compiles every declared program, by a run against empty facts (2–6 ms) when no invariant uses it. There is no compile-only operation.
- For each run the host generates a wrapper program in the private directory. It includes the consumer's program and holds one `.input` directive for each fact relation and supplied input and one `.output` directive for each relation that a configuration entry names in that program, all as JSON files in that directory (`IO=jsonfile`, `format=list`). JSON keeps tabs, newlines and quotes in values, which Soufflé's default tab-separated files do not.
- Native: `souffle --no-preprocessor -l "" <wrapper>.dl` as a child process, single-threaded, with the private directory as its working directory. `-l ""` stops Soufflé from loading a `libfunctors.so`, which it otherwise does for every program.
- Browser: Soufflé runs in a worker of its own, apart from the one that holds the workspace. Compile `souffle.wasm` once per such worker. For each evaluation, create a fresh instance, write programs and facts into its in-memory filesystem and read the outputs back. The module is built with `-sEXIT_RUNTIME=1` or exports `callMain`, so the host receives the exit code of `main`.
- Every consumer of a program reads the same run: the outputs do not depend on which consumer asked.
- Numbers are signed 32-bit integers on every host. A row in an output file that does not fit the relation's columns, or a value that is not valid UTF-8 (`substr` counts bytes), is a fault.
- Explanations run on demand in provenance mode (`-t explain`), driven on stdin by `format json`, then `explain <relation>(<args>)`. Use JSON: the text proof format garbles negated atoms. In the browser the host supplies that input through the module's `stdin` hook. When provenance mode aborts, the host reports the explanation as unavailable.
- The `explain` command does not decode escapes, and a default-depth proof holds unresolved `subproof` references. The host expands them, checks that the proof it gets back is for the row it asked about, and reports the explanation as unavailable when the row's values cannot be written in the command (quotes, backslashes, control characters, negative numbers), when the reply is not valid JSON, when a branch ends in an aggregate's internal relation, or when the exchange passes the time limit. It leaves constraint leaves out: Soufflé prints internal numbers for symbols there. A leaf from a supplied input names that input and holds the row's values as supplied, instead of a location.
- A runtime fault such as `to_number` of text that is not a number ends the child process with the rule's location natively, and aborts the module without a location in WASM.
- Soufflé's compile diagnostics name a file and a line. Anything else it prints on stderr during a run, such as the warning for a `substr` out of range, is a runtime fault, also when it exits with 0.
- A compile error, a refusal or a fault is an error diagnostic at the project path of the file it concerns, never the private copy's, and at its line; a compile error's message ends with the source line and a caret under the column. A fault without a line, or from a limit, which it names, is at the configuration line that declares the program. `project validate` is then invalid, `query execute` and `query view` fail, and a gate and every item it names are unavailable.
- At the time limit ([REQ-EVALUATION-TIME-LIMIT](#01M7CMW70G711Z67QEWW5Z0MK0)) the host kills the child process natively and terminates the Soufflé worker in the browser; the workspace and its unexported edits are not in that worker.

---

## Evidence

---

<a id="01M7BF6HDDJJ0Y0WWT6RK17CDN"></a>

### Soufflé trial

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-TRIAL |
| Status | Accepted |
| Supports | [ADR-SOUFFLE-DATALOG](decisions-rules.mara.md#01M7BF6GSZ16MZQAGBY5M9TDKZ) |
| Supports | [ADR-SOUFFLE-AS-PROCESS](decisions-rules.mara.md#01M7BF6GT9PVXSKV1ANGEWA7WR) |

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
| Supports | [ADR-PROGRAM-PROFILE](decisions-rules.mara.md#01M7CMW6ZGC0PY8DVYNQDSBPRH) |
| Supports | [RISK-SOUFFLE-REACHES-OUTSIDE](rules-facts.mara.md#01M7CYC90BZBBXGC0CZ66W8ZEV) |

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
| `nat(0). nat(x + 1) :- nat(x).` | Still running when killed after 5 s |
| `s("a"). s(cat(x, "a")) :- s(x).` | Still running when killed after 5 s |

The two programs that never ended use no records, ADTs or `autoinc()`.
