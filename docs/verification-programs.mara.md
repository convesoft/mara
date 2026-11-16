# Program safety verification

---

<a id="01M7HZYKB4R632Q63YWS8CY1FJ"></a>

### Report every program fault as unavailable

| Type | Verification |
| --- | --- |
| ID | VER-PROGRAM-FAULTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81) |

In the setup of [VER-PROGRAM-CONSUMERS](verification-rules.mara.md#01M7HZYK9DYQDY8XFKWZES1ZB6), each faulty program is consumed by an invariant of each severity, a query and a gate that names two items, and the tests run `project validate`, `query execute` and `query gate`. Each fault is reported as [DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) says: an error diagnostic at the project path of the file and at the line, never the private copy's path; `project validate` invalid with exit status 1; `query execute` failed with exit status 1; the gate and both items unavailable.

| Case | Expectation |
| --- | --- |
| An undefined relation, a wrong arity, a `Mid` used as an `Id`, a symbol used as a number, and negation that cannot be stratified ([EVD-SOUFFLE-TRIAL](rules-evaluation.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN)), in the declared program and in a declared program it includes | Reported at the file and line that hold the error; the message ends with that source line and a caret under the column |
| A program the parser refuses ([VER-ADMITTED-LANGUAGE](#01M7HZYKDB9NDNWP4KJFGK5A9D)) | Reported the same way, at the place the parser stopped; 0 subprocess spawns |
| `to_number` of a field value `abc` | A runtime fault at the line of the rule |
| `substr` of a title with a start past its end | A runtime fault, although `souffle` exits with 0 |
| `substr` that splits a two-byte character of a title, in a consumed relation | A fault: the output is not valid UTF-8 |
| `to_number("7junk")`; `substr("abcdef", 2, -1)` | `7`; `cdef`: no fault |
| A consumed relation whose output would be at least twice the output-size limit that DES-RULE-EXECUTION records | A fault that names that limit; no row returned |
| A program that holds at least twice the memory limit that DES-RULE-EXECUTION records, with one output row per item: a `count` over a relation of all pairs of a `range` | A fault that names that limit, reported before the time limit |
| From 0.4.0-alpha.6 (mara-xsm8w6), each runtime-fault case in the browser | Unavailable; a fault without a line is reported at the line of the configuration that declares the program |

---

<a id="01M7HZYKBNDY3ZA77JMB0VD06H"></a>

### Stop an evaluation at its time limit

| Type | Verification |
| --- | --- |
| ID | VER-EVALUATION-TIME-LIMIT |
| Status | Draft |
| Methods | Test |
| Verifies | [REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0) |

Draft with its requirement, whose number is provisional. In the setup of [VER-PROGRAM-FAULTS](#01M7HZYKB4R632Q63YWS8CY1FJ), a stopped evaluation is reported as a fault that names the time limit. The long program is a `count` over the four-fold product of a `range` of 1,000 numbers: it needs hours, little memory and one output row per item.

| Case | Expectation |
| --- | --- |
| The long program, with a limit of 1 s set in the configuration | The request ends after at least 1 s and within 10 s; no `souffle` process with its working directory under the test's temporary directory remains, and `TMPDIR` holds what it held before |
| The long program, without a configured limit | The request ends after at least 10 s and within 20 s |
| `nat(0). nat(x + 1) :- nat(x).` and `s("a"). s(cat(x, "a")) :- s(x).` ([EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)), with the 1 s limit | Stopped by the time limit or the memory limit, whichever comes first |
| `query report` of a row of an invariant over the long program, with the 1 s limit | Cannot explain, naming the time limit |
| From 0.4.0-alpha.6 (mara-xsm8w6), the first case in the browser | The Soufflé worker is terminated; the workspace keeps every unexported edit |

---

<a id="01M7HZYKCS3TN1SR30YSN69SED"></a>

### Reproduce every way a program reached outside

| Type | Verification |
| --- | --- |
| ID | VER-PROGRAM-REACH-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROGRAMS-ONLY-COMPUTE](rules.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Each row of [EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG) is a case. In the setup of [VER-PROGRAM-CONSUMERS](verification-rules.mara.md#01M7HZYK9DYQDY8XFKWZES1ZB6), each case declares one program, consumed by an invariant and a query, and runs `project validate` and `query execute`. The test's temporary directory also holds a file `secret` with a marker line, and a `libfunctors.so` that the test builds with `cc`, whose constructor creates a marker file. After every case no marker file exists, neither marker is on stdout or stderr, and every file under the test's temporary directory, `TMPDIR` included, has the name, bytes and mtime it had before. Refused means reported as [VER-PROGRAM-FAULTS](#01M7HZYKB4R632Q63YWS8CY1FJ) requires, with 0 subprocess spawns.

| Program | Expectation |
| --- | --- |
| `.output r(IO=file, filename="<absolute path>")`; `.input s(IO=file, filename="<secret>")` | Refused |
| `.functor f(x: number): number`, the library in the program's directory | Refused |
| `.pragma "library-dir"` and `.pragma "libraries"` naming the library; `.pragma "output-dir"` | Refused |
| An admitted program, the library in the program's directory, in the working directory of `mara`, and in a directory on `LD_LIBRARY_PATH` | Its rows; the library is not loaded |
| `#line 1 "<secret>"` and `# 1 "<secret>"` before a faulty declaration: at the start of a line, after other text on it, after a block comment | Refused |
| `__FILE__` in a fact; `__INCL__` | Refused |
| A value holding a newline and a tab | One row with that value ([VER-FACT-ROWS](verification-rules.mara.md#01M7HZYK9ZXDEW7JNRV16GG1D5)) |
| `.decl r(x: symbol) output printsize`; the `input` qualifier | Refused |
| `choice-domain` | Refused |
| A `float` column; an input row of `1.5` or `"not-a-number"` for a number column | Refused; refused, naming the row and column ([VER-REQUEST-INPUTS](verification-rules.mara.md#01M7HZYKAH5ZEC7HMT3D0RYTP2)) |
| `2 ^ 40`; `match("[", m)` and `match("\\cI", "\t")`; `as(n, symbol)`; `x % y`; `x / 0` | Refused |
| `+`, `*` and shifts at the limits of 32 bits, `strlen`, `to_number` of a hexadecimal string | The rows the review probes recorded natively; from 0.4.0-alpha.6 (mara-xsm8w6), the same rows in the browser |
| `/` and `%` at those limits, division by zero | Refused |
| A `magic` relation with a subsumption clause | Refused |
| `to_number(2147483648.0)` | Refused |
| `-t explain` on `autoinc()`, an ADT constructor or a subsumption clause | Refused before any run |
| The explanation rows: an escaped newline, `x < "z"`, records, aggregates, negation, components, `eqrel` | Cases of [VER-EXPLANATIONS](verification-rules.mara.md#01M7HZYKC7RWFY0258CAY3SRET) |
| The two programs that never end | Cases of [VER-EVALUATION-TIME-LIMIT](#01M7HZYKBNDY3ZA77JMB0VD06H) |
| The upstream row | No program: each construct it names is a case above |

---

<a id="01M7HZYKDB9NDNWP4KJFGK5A9D"></a>

### Parse the admitted language and nothing else

| Type | Verification |
| --- | --- |
| ID | VER-ADMITTED-LANGUAGE |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROGRAMS-ONLY-COMPUTE](rules.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Tests of the engine crate's parser run on the review probe programs and the trial programs of [EVD-SOUFFLE-TRIAL](rules-evaluation.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN) rewritten in the admitted language, both copied into the repository as fixtures (mara-wrbjte). A refusal names the line and column where the parser stopped.

| Case | Expectation |
| --- | --- |
| Each construct that [DES-PROGRAM-PROFILE](rules-facts.mara.md#01M7CMW7051QVPFDWQBMB1A6K6) leaves out, in each spelling of the probes: `#line` and `#` at the start of a line, after other text and after a block comment; the `input`, `output` and `printsize` qualifiers; `float` and `unsigned` literals without the type's name; `as`; `magic`; subsumption; `match`; `/`; `%`; `^` | Refused at its token |
| An integer literal outside signed 32 bits, decimal, hexadecimal or binary | Refused |
| `.include` of an absolute path, of a path with `..`, and of a project file that no program pattern declares | Refused |
| A fact relation, supplied input or consumed relation with a record column, also through a subtype, a union or a component's type parameter | Refused; the same record type in a relation that does not cross the boundary is admitted |
| `min` and `max` aggregates over symbols | Refused |
| The variadic `min(...)` and `max(...)` over symbols; `count` over symbol tuples | Admitted |
| A left-out word inside a comment or a string literal | Admitted |
| Every probe program that the parser admits | `souffle` compiles it: a run against empty facts exits 0 with no compile diagnostic |
| The rewritten trial programs | Admitted, and they compile |
