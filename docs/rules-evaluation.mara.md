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

For any row of an invariant or gate outcome, Mara returns an explanation whose leaves are input facts with document path and line.

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
| Satisfies | [REQ-EVALUATION-TIME-LIMIT](#01M7CMW70G711Z67QEWW5Z0MK0) |

- An evaluation is keyed by document snapshot key, program hash and parameter set ([DES-INPUTS-AND-KEYS](engine.mara.md#01M7BF6GYCNTZQ0VXCMGJYPZHY)).
- It runs only programs that pass the profile scan ([DES-PROGRAM-PROFILE](rules.mara.md#01M7CMW7051QVPFDWQBMB1A6K6)).
- Native: `souffle --no-preprocessor -F <facts> -D <out> <program>.dl` as a child process, single-threaded.
- Browser: compile `souffle.wasm` once per worker. For each evaluation, create a fresh instance, write programs and facts into its in-memory filesystem and read the outputs back. The module is built with `-sEXIT_RUNTIME=1` or exports `callMain`, so the host receives the exit code of `main`.
- Mara reads only the output relations a consumer names. An `.output` in an included file is also written and is not a result.
- Explanations run on demand in provenance mode (`-t explain`), driven on stdin by `format json`, then `explain <relation>(<args>)`. Use JSON: the text proof format garbles negated atoms.
- A runtime fault such as integer division by zero ends the child process with the rule's location natively, and with a `divide by zero` trap without location in WASM.
- At the time limit the host kills the child process natively and terminates the worker in the browser.

---

## Evidence

---

<a id="01M7BF6HDDJJ0Y0WWT6RK17CDN"></a>

### Soufflé trial

| Type | Evidence |
| --- | --- |
| ID | EVD-SOUFFLE-TRIAL |
| Status | Accepted |
| Supports | [ADR-SOUFFLE-DATALOG](decisions-runtime.mara.md#01M7BF6GSZ16MZQAGBY5M9TDKZ) |
| Supports | [ADR-SOUFFLE-AS-PROCESS](decisions-runtime.mara.md#01M7BF6GT9PVXSKV1ANGEWA7WR) |

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
| Supports | [ADR-PROGRAM-PROFILE](decisions-runtime.mara.md#01M7CMW6ZGC0PY8DVYNQDSBPRH) |

The static binary of [EVD-SOUFFLE-TRIAL](#01M7BF6HDDJJ0Y0WWT6RK17CDN), run on 2026-11-14 as `souffle --no-preprocessor -F <facts> -D <out> <program>.dl`.

| Program | Result |
| --- | --- |
| `.output r(IO=file, filename="<absolute path>")` | Wrote that file, outside `<out>` |
| `.input s(IO=file, filename="/etc/hostname")` | Read that file into `s` |
| `.functor f(x: number): number`, with a `libfunctors.so` in the working directory | Loaded the library and ran its code |
| `.pragma "library-dir"` and `.pragma "libraries"` naming a library elsewhere | Loaded that library and ran its code |
| `.pragma "output-dir"` | No effect; `-D` won |
| `nat(0). nat(x + 1) :- nat(x).` | Still running when killed after 5 s |
| `s("a"). s(cat(x, "a")) :- s(x).` | Still running when killed after 5 s |

The last two programs use no records, ADTs or `autoinc()`.

---

## Open questions

- Explanations in the browser: the trial drove the `explain` console on stdin only natively.
