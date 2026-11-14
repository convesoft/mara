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

- `mara.dl` only declares the relations. For each run Mara writes their facts, one JSON file per relation and once per document snapshot, and generates the directives that read them ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).
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
| Mitigates | [RISK-SOUFFLE-REACHES-OUTSIDE](#01M7CYC90BZBBXGC0CZ66W8ZEV) |

Before every run the host copies `mara.dl` and the declared program files into a private directory, scans each copy outside comments and strings, and runs Soufflé on the copies. A program passes only when each of its directives is admitted:

| Directive | Admitted |
| --- | --- |
| `.decl`, `.type`, `.comp`, `.init`, `.override`, `.plan`, `.once`, `.limitsize` | Always |
| `.include` | Of `mara.dl` and the declared programs |

- Every other directive is refused, `.functor`, `.pragma` and `.lattice` among them. So are `.input`, `.output` and `.printsize`: a program only declares relations and rules, and Mara generates its input and output.
- A `#` outside strings and comments is refused wherever it stands: `#line` makes a diagnostic print the first line of any file, also with the preprocessor off and also after other text on the line.
- `__FILE__` and `__INCL__` are refused: they yield the private directory's path, which differs from run to run.
- The scan follows Soufflé's own lexical rules: a directive may follow other text on a line, and block comments do not nest.
- `autoinc()` is refused: its values depend on evaluation order, and it aborts explanations.
- A refusal is reported like a compile error ([REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)).
- Records and ADTs are admitted. Time is bounded instead ([REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)).
- Soufflé runs with library loading off and never in the project's directory ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).

---

## Risks

---

<a id="01M7CYC90BZBBXGC0CZ66W8ZEV"></a>

### A program reaches outside its evaluation

| Type | Risk |
| --- | --- |
| ID | RISK-SOUFFLE-REACHES-OUTSIDE |
| Status | Accepted |
| Likelihood | Unlikely |
| Consequence | Severe |
| Threatens | [REQ-PROGRAMS-ONLY-COMPUTE](rules.mara.md#01M7CMW6ZTAJHSF5BR9SF9KR86) |

A construct that the scan admits, or a memory fault in Soufflé, lets a program from an untrusted checkout read or write files or run code as the user who validates it. Two reviews each found one way past the earlier scan ([EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).

---

## Open questions

- Whether the `souffle` child process also runs in an operating-system sandbox, or an accepted Decision accepts [RISK-SOUFFLE-REACHES-OUTSIDE](#01M7CYC90BZBBXGC0CZ66W8ZEV), and whether an evaluation gets limits on output size and memory besides time. The maintainer decides before the release.
