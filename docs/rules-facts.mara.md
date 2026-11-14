# Rule facts and programs

## Design

---

<a id="01M7BF6HEHHJ7XZM8HDFXK115J"></a>

### Fact bridge

| Type | Design |
| --- | --- |
| ID | DES-FACT-BRIDGE |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](rules.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |
| Satisfies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](rules.mara.md#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

Mara ships a versioned declarations file, `mara.dl`, that starts with `.once`; programs include it with `.include "mara.dl"`. It declares only relations that every vocabulary has. Trial version:

```souffle
.type Mid <: symbol
.type Id <: symbol
.type Type <: symbol
.type Status <: symbol
.type Rel <: symbol
.type Path <: symbol

.decl item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
.decl title(mid: Mid, text: symbol)
.decl revision(mid: Mid, hash: symbol)
.decl field(mid: Mid, key: symbol, value: symbol)
.decl folded(text: symbol, folded: symbol)
.decl edge(rel: Rel, src: Mid, dst: Mid, path: Path, line: number)
.decl source_complete(source: symbol)
```

- Each relation is `.input`. Mara writes one TSV file per relation, once per document snapshot.
- Types, statuses, field keys, enum values and relation names are schema names (`requirement`, `derives_from`), never labels.
- The distinct subtypes `Mid`, `Id` and `Path` make Soufflé reject rules that mix identities.
- `revision` is the hash of the item's region. `get` returns the same value, so whoever records a result can record the definition it belongs to.
- `folded` gives the NFC, case-folded form of every title and field value, as search normalizes text. Soufflé has no case folding.
- `edge` rows carry the location of their metadata row.
- `source_complete` names a source whose records are all present ([DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).
- Code links ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) become a further input relation.
- The file's format version starts at 1 and changes only with a released contract change, never for internal iterations.

---

<a id="01M7CMW7051QVPFDWQBMB1A6K6"></a>

### Admitted program profile

| Type | Design |
| --- | --- |
| ID | DES-PROGRAM-PROFILE |
| Status | Accepted |
| Satisfies | [REQ-PROGRAMS-ONLY-COMPUTE](rules.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Before every run the host scans each program file, outside comments and strings, and refuses:

| Construct | It would |
| --- | --- |
| `.functor` | Load a shared library and run its code |
| `.pragma` | Set Soufflé options, library paths included |
| `.input`, `.output` or `.printsize` with parameters | Read or write any file or database |
| `.include` of a file other than `mara.dl` and the declared programs | Read an undeclared file |

- A refusal is reported like a compile error ([REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)).
- Everything else in Soufflé is admitted, records, ADTs and `autoinc()` included. Time is bounded instead ([REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)).
