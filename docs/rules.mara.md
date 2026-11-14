# Rules

## Requirements

---

<a id="01M7BF6HDMR4G13G7VFDA770ZN"></a>

### Projects write their own logic

| Type | Requirement |
| --- | --- |
| ID | REQ-PROJECT-AUTHORED-LOGIC |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

A project defines invariants, named queries, gates and views as Soufflé Datalog programs over the facts Mara supplies. Adding or changing a program needs no compiler and no change to Mara.

---

<a id="01M7BF6HDVZSZM8E9BAEF7NNQ0"></a>

### External records are evaluated as supplied

| Type | Requirement |
| --- | --- |
| ID | REQ-EXTERNAL-RECORDS-AS-SUPPLIED |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

Mara evaluates externally supplied records, such as verification results, as given: it never runs tests or vouches for their truth. A record counts as missing only when its source is marked complete; otherwise the outcome is unavailable.

---

<a id="01M7CMW6ZTAJHSF5BR9SF9KR86"></a>

### Programs only compute

| Type | Requirement |
| --- | --- |
| ID | REQ-PROGRAMS-ONLY-COMPUTE |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

Evaluating a program reads only the facts Mara supplies, writes only the outputs Mara collects and loads no native code. A program that could do otherwise is refused before it runs, and its outcome is unavailable.

---

## Design

---

<a id="01M7BF6HE529B61Q2ZNXFQ1JXE"></a>

### Programs and their consumers

| Type | Design |
| --- | --- |
| ID | DES-RULE-CONSUMERS |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](#01M7BF6HDMR4G13G7VFDA770ZN) |

One program's output relations serve several consumers; presentation never defines different facts.

| Consumer | Defined by | Effect |
| --- | --- | --- |
| Raw query | A request that names a declared program and one of its output relations | Returns the output rows |
| View | A configuration entry: program and relation | Renders the same rows |
| Invariant | A configuration entry: program, relation, severity and message | Evaluated by validation; an error-severity failure makes documentation invalid |
| Gate | A configuration entry: program, condition relations and parameters | Requested by name for a scope and context; a failure fails the gate and never changes documentation validity |
| Report | A request that names an invariant or a gate | Explains its outcome |

- Each row of an invariant relation or of a gate's condition relation is one failure. Its first column is the MID of the item concerned, which gives the failure its path and line.
- An empty result has no pass or fail meaning of its own; the consumer defines it.
- A gate reports conditions, not permission to release, deploy or act.

```toml
[rules]
programs = [".mara/rules/*.dl"]

[invariants.requirement-verified]
program = ".mara/rules/engineering.dl"
relation = "unverified_requirement"
severity = "error"
message = "Accepted requirement has no accepted verification"

[gates.implementation-ready]
program = ".mara/rules/gates.dl"
conditions = ["not_accepted", "blocking_risk"]
parameters = ["threshold"]

[views.coverage]
program = ".mara/rules/engineering.dl"
relation = "coverage"
```

---

<a id="01M7BF6HEHHJ7XZM8HDFXK115J"></a>

### Fact bridge

| Type | Design |
| --- | --- |
| ID | DES-FACT-BRIDGE |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](#01M7BF6HDMR4G13G7VFDA770ZN) |
| Satisfies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

Mara ships a versioned declarations file, `mara.dl`, that starts with `.once`; programs include it with `.include "mara.dl"`. Trial version:

```souffle
.type Mid <: symbol
.type Id <: symbol
.type Type <: symbol
.type Status <: symbol
.type Rel <: symbol
.type Path <: symbol

.decl item(mid: Mid, type: Type, id: Id, status: Status, path: Path, line: number)
.decl field(mid: Mid, key: symbol, value: symbol)
.decl edge(rel: Rel, src: Mid, dst: Mid, path: Path, line: number)
.decl verification_result(ver: Mid, result: symbol, revision: symbol)
.decl source_complete(source: symbol)
```

- Each relation is `.input`. Mara writes one TSV file per relation, once per document snapshot.
- Types, statuses, field keys, enum values and relation names are schema names (`requirement`, `derives_from`), never labels.
- The distinct subtypes `Mid`, `Id` and `Path` make Soufflé reject rules that mix identities.
- `edge` rows carry the location of each occurrence, so a relation authored twice yields two rows. Programs that count distinct relationships project away `path` and `line`.
- `source_complete` marks an externally supplied source as complete.
- A gate parameter is a one-row input relation, for example `threshold(value: number)`.
- Code links ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) become a further input relation.
- The file's format version starts at 1 and changes only with a released contract change, never for internal iterations.

---

<a id="01M7CMW7051QVPFDWQBMB1A6K6"></a>

### Admitted program profile

| Type | Design |
| --- | --- |
| ID | DES-PROGRAM-PROFILE |
| Status | Accepted |
| Satisfies | [REQ-PROGRAMS-ONLY-COMPUTE](#01M7CMW6ZTAJHSF5BR9SF9KR86) |

Before every run the host scans each program file, outside comments and strings, and refuses:

| Construct | It would |
| --- | --- |
| `.functor` | Load a shared library and run its code |
| `.pragma` | Set Soufflé options, library paths included |
| `.input`, `.output` or `.printsize` with parameters | Read or write any file or database |
| `.include` of a file other than `mara.dl` and the declared programs | Read an undeclared file |

- A refusal is reported like a compile error ([REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)).
- Everything else in Soufflé is admitted, records, ADTs and `autoinc()` included. Time is bounded instead ([REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)).

---

## Open questions

- The scope and context of a gate, how a program reports an unavailable outcome, and how external records are supplied.
