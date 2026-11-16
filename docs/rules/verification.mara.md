# Rules verification

---

<a id="01M7HZYK9DYQDY8XFKWZES1ZB6"></a>

### Run project programs through every consumer

| Type | Verification |
| --- | --- |
| ID | VER-PROGRAM-CONSUMERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](index.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |

Rust integration tests run the real CLI on disposable projects of the `empty` template with a small hand-written schema, programs under `.mara/rules/` and their entries in the configuration ([DES-RULE-CONSUMERS](index.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)). The binary runs with an environment that holds only `HOME` and `TMPDIR`, both inside the test's temporary directory, and finds the pinned `souffle` from its own path ([DES-SOUFFLE-BUILD](../distribution.mara.md#01M7BF6GX9WNFK70C6H1RBQJ5R)): no compiler is reachable. The other rules Verification items use this setup. Every consumed relation in them starts with the MID of the item its row is about, as DES-RULE-CONSUMERS requires; a row described without one is joined to an item of the project.

| Case | Expectation |
| --- | --- |
| `query list` | Every query, view, invariant and gate entry with its name, description and inputs, and nothing else |
| `query execute` and `query view` of two entries on the same relation | The same rows |
| `query execute` naming two items | Only the rows whose first column is the MID of one of them |
| `query execute` of a name that no entry has, such as a relation of the program | Refused, naming it; 0 program runs |
| An error-severity invariant whose relation has two rows | `project validate`: one error per row, with the invariant's name as code and its message, at the path and line and with the ID and MID of the item in the row's first column; invalid, exit status 1 |
| The same invariant with severity `warning` | Two warnings; valid, exit status 0 |
| An invariant whose relation is empty | No diagnostic from it |
| A rule added to a program between two requests | The second request's rows follow the new rule |
| A gate over four named items: one without a subject row; three with one, of which the first also has a condition row and an unavailable row, the second only an unavailable row and the third nothing else | Not applicable, failed, unavailable and passed, each listed with the rows that caused it; the gate failed ([DES-GATE-OUTCOME](index.mara.md#01M7CT60KEEZRJCPJ0CPT7YT5B)) |
| The same gate naming only the last two items; only the last | Unavailable; passed |
| A gate request that names no item | Refused; 0 program runs |
| `project validate` before and after a failed gate request | The same result |

---

<a id="01M7HZYK9ZXDEW7JNRV16GG1D5"></a>

### Read the supplied facts back through a program

| Type | Verification |
| --- | --- |
| ID | VER-FACT-ROWS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](index.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), a program includes `mara.dl` and copies each of its relations into a consumed relation that starts with an item's MID; `query execute` returns them ([DES-FACT-BRIDGE](index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)). `item`, `title`, `revision` and `field` are copied with the same columns. `edge` is copied as source MID, relation, target MID, path and line; `folded` is joined to each title and field value it folds, as MID, original and folded text; `source_complete` and a supplied input are joined to every `item`, as MID and the relation's columns. The expectations compare the fact columns after these projections. The project has items of two types and statuses in two documents, a field, and relation rows in the metadata tables of two items.

| Case | Expectation |
| --- | --- |
| `item`, `title`, `field` and `edge` rows | One per item, title, field row and relation row, with the values that `get` and `relation list` give: schema names for type, status, key and relation, the item's anchor line, and for `edge` the path and line of its relation row |
| `revision` | One row per item, equal to the `revision` that `get` returns |
| `folded` of a title `ÉCOLE` written with a combining accent | `école`, with a precomposed `é`, on that item's row |
| A title and a field value holding a tab, a quote, a backslash and non-ASCII text | Returned as `get` gives them, each value one row |
| A supplied input value holding a newline and a tab ([VER-REQUEST-INPUTS](#01M7HZYKAH5ZEC7HMT3D0RYTP2)) | Returned unchanged, one row per item |
| `source_complete` without `--complete`; with `--complete` of an input the entry lists | No row; one row per item, naming that input |
| A rule that compares a `Mid` with an `Id` | A compile error ([VER-PROGRAM-FAULTS](#01M7HZYKB4R632Q63YWS8CY1FJ)) |
| From 0.4.0-alpha.6 (mara-xsm8w6), each case in the browser's Soufflé | The same rows |

---

<a id="01M7HZYKAH5ZEC7HMT3D0RYTP2"></a>

### Supply external records to a gate

| Type | Verification |
| --- | --- |
| ID | VER-REQUEST-INPUTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](index.mara.md#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), a gate lists the inputs `candidate` (one symbol column) and `result` (two symbols and a number). A named item with a `fail` row in `result` is in its condition relation. A named item without a `result` row is in its unavailable relation, or in its condition relation when `source_complete("result")` holds ([DES-REQUEST-INPUTS](index.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).

| Case | Expectation |
| --- | --- |
| `--input candidate=abc` and `--input result=@results.json` | The program reads exactly those rows; the `fail` row fails its item as supplied |
| The item without a result, without `--complete result`; with it | Unavailable; failed |
| The same rows passed as rows through MCP | The CLI's result ([VER-CLI-MCP-PARITY](../hosts/verification.mara.md#01M7F74J5Z4KX9F3HN78T1WB85)) |
| A listed input omitted; an input that the entry does not list | Refused, naming it; 0 program runs |
| A row with too few or too many columns, a number for a symbol, a string, `1.5`, `2147483648` or `-2147483649` for a number, and a file that is no JSON list of lists | Refused, naming the row and column; 0 program runs |
| `-2147483648` and `2147483647` for a number | Read as given |
| Every request above | `--debug-stats` counts as many subprocess spawns as program runs: Mara starts no other process |
| A later request without the inputs | Refused as omitting them: nothing was stored; the project tree is unchanged |

---

<a id="01M7HZYKC7RWFY0258CAY3SRET"></a>

### Explain failing rows down to their lines

| Type | Verification |
| --- | --- |
| ID | VER-EXPLANATIONS |
| Status | Accepted |
| Methods | Test |
| Methods | Inspection |
| Verifies | [REQ-EXPLAIN-OUTCOMES](evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), `query report` explains rows of invariants and gates ([DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)). Each leaf of a fact relation is compared with what `get` and `relation list` give for its item or relation row; a leaf of a supplied input names that input and holds the row's values as supplied ([REQ-EXPLAIN-OUTCOMES](evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE)).

| Case | Method | Expectation |
| --- | --- | --- |
| A row of an invariant that joins `item`, `field` and `edge`, and a condition row of a gate that reads a supplied input | Test | A proof of that row whose leaves are input facts, each fact leaf at the path and line of its item or relation row, and the input's leaf with its name and the supplied row's values |
| Rows whose proofs pass a negated atom, a record, a component and an `eqrel` relation | Test | Explained |
| A row of an ancestry closure over a chain of 8 `Decomposes` links | Test | A proof whose leaves include the 8 `edge` rows, with no `subproof` reference left |
| A rule with a comparison of symbols, `x < "z"` | Test | Explained, with no constraint leaf |
| Rows whose value holds a newline from a supplied input, a quote or a backslash, or is a negative number | Test | Cannot explain; never a proof of another row |
| A row whose proof passes an aggregate | Test | Cannot explain |
| Each case | Test | `project validate` and the gate return the same result as without the report |
| The check that a returned proof is for the requested row, and the handling of a reply that is not JSON and of an aborted provenance run | Inspection | Review of mara-bx82k0 finds each; no admitted program is known to reach them ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)) |
| From 0.4.0-alpha.6 (mara-xsm8w6), the test cases in the browser | Test | The same proofs |

---

<a id="01M7HZYKB4R632Q63YWS8CY1FJ"></a>

### Report every program fault as unavailable

| Type | Verification |
| --- | --- |
| ID | VER-PROGRAM-FAULTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROGRAM-FAULTS-REPORTED](evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), each faulty program is consumed by an invariant of each severity, a query and a gate that names two items, and the tests run `project validate`, `query execute` and `query gate`. Each fault is reported as [DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH) says: an error diagnostic at the project path of the file and at the line, never the private copy's path; `project validate` invalid with exit status 1; `query execute` failed with exit status 1; the gate and both items unavailable.

| Case | Expectation |
| --- | --- |
| An undefined relation, a wrong arity, a `Mid` used as an `Id`, a symbol used as a number, and negation that cannot be stratified ([EVD-SOUFFLE-TRIAL](evaluation.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN)), in the declared program and in a declared program it includes | Reported at the file and line that hold the error; the message ends with that source line and a caret under the column |
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
| Verifies | [REQ-EVALUATION-TIME-LIMIT](evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0) |

Draft with its requirement, whose number is provisional. In the setup of [VER-PROGRAM-FAULTS](#01M7HZYKB4R632Q63YWS8CY1FJ), a stopped evaluation is reported as a fault that names the time limit. The long program is a `count` over the four-fold product of a `range` of 1,000 numbers: it needs hours, little memory and one output row per item.

| Case | Expectation |
| --- | --- |
| The long program, with a limit of 1 s set in the configuration | The request ends after at least 1 s and within 10 s; no `souffle` process with its working directory under the test's temporary directory remains, and `TMPDIR` holds what it held before |
| The long program, without a configured limit | The request ends after at least 10 s and within 20 s |
| `nat(0). nat(x + 1) :- nat(x).` and `s("a"). s(cat(x, "a")) :- s(x).` ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)), with the 1 s limit | Stopped by the time limit or the memory limit, whichever comes first |
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
| Verifies | [REQ-PROGRAMS-ONLY-COMPUTE](index.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Each row of [EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG) is a case. In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), each case declares one program, consumed by an invariant and a query, and runs `project validate` and `query execute`. The test's temporary directory also holds a file `secret` with a marker line, and a `libfunctors.so` that the test builds with `cc`, whose constructor creates a marker file. After every case no marker file exists, neither marker is on stdout or stderr, and every file under the test's temporary directory, `TMPDIR` included, has the name, bytes and mtime it had before. Refused means reported as [VER-PROGRAM-FAULTS](#01M7HZYKB4R632Q63YWS8CY1FJ) requires, with 0 subprocess spawns.

| Program | Expectation |
| --- | --- |
| `.output r(IO=file, filename="<absolute path>")`; `.input s(IO=file, filename="<secret>")` | Refused |
| `.functor f(x: number): number`, the library in the program's directory | Refused |
| `.pragma "library-dir"` and `.pragma "libraries"` naming the library; `.pragma "output-dir"` | Refused |
| An admitted program, the library in the program's directory, in the working directory of `mara`, and in a directory on `LD_LIBRARY_PATH` | Its rows; the library is not loaded |
| `#line 1 "<secret>"` and `# 1 "<secret>"` before a faulty declaration: at the start of a line, after other text on it, after a block comment | Refused |
| `__FILE__` in a fact; `__INCL__` | Refused |
| A value holding a newline and a tab | One row with that value ([VER-FACT-ROWS](#01M7HZYK9ZXDEW7JNRV16GG1D5)) |
| `.decl r(x: symbol) output printsize`; the `input` qualifier | Refused |
| `choice-domain` | Refused |
| A `float` column; an input row of `1.5` or `"not-a-number"` for a number column | Refused; refused, naming the row and column ([VER-REQUEST-INPUTS](#01M7HZYKAH5ZEC7HMT3D0RYTP2)) |
| `2 ^ 40`; `match("[", m)` and `match("\\cI", "\t")`; `as(n, symbol)`; `x % y`; `x / 0` | Refused |
| `+`, `*` and shifts at the limits of 32 bits, `strlen`, `to_number` of a hexadecimal string | The rows the review probes recorded natively; from 0.4.0-alpha.6 (mara-xsm8w6), the same rows in the browser |
| `/` and `%` at those limits, division by zero | Refused |
| A `magic` relation with a subsumption clause | Refused |
| `to_number(2147483648.0)` | Refused |
| `-t explain` on `autoinc()`, an ADT constructor or a subsumption clause | Refused before any run |
| The explanation rows: an escaped newline, `x < "z"`, records, aggregates, negation, components, `eqrel` | Cases of [VER-EXPLANATIONS](#01M7HZYKC7RWFY0258CAY3SRET) |
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
| Verifies | [REQ-PROGRAMS-ONLY-COMPUTE](index.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Tests of the engine crate's parser run on the review probe programs and the trial programs of [EVD-SOUFFLE-TRIAL](evaluation.mara.md#01M7BF6HDDJJ0Y0WWT6RK17CDN) rewritten in the admitted language, both copied into the repository as fixtures (mara-wrbjte). A refusal names the line and column where the parser stopped.

| Case | Expectation |
| --- | --- |
| Each construct that [DES-PROGRAM-PROFILE](index.mara.md#01M7CMW7051QVPFDWQBMB1A6K6) leaves out, in each spelling of the probes: `#line` and `#` at the start of a line, after other text and after a block comment; the `input`, `output` and `printsize` qualifiers; `float` and `unsigned` literals without the type's name; `as`; `magic`; subsumption; `match`; `/`; `%`; `^` | Refused at its token |
| An integer literal outside signed 32 bits, decimal, hexadecimal or binary | Refused |
| `.include` of an absolute path, of a path with `..`, and of a project file that no program pattern declares | Refused |
| A fact relation, supplied input or consumed relation with a record column, also through a subtype, a union or a component's type parameter | Refused; the same record type in a relation that does not cross the boundary is admitted |
| `min` and `max` aggregates over symbols | Refused |
| The variadic `min(...)` and `max(...)` over symbols; `count` over symbol tuples | Admitted |
| A left-out word inside a comment or a string literal | Admitted |
| Every probe program that the parser admits | `souffle` compiles it: a run against empty facts exits 0 with no compile diagnostic |
| The rewritten trial programs | Admitted, and they compile |
