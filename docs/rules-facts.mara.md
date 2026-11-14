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

Before every run the host copies `mara.dl` and the declared program files into a private directory, parses each copy with Mara's own parser for the language below, resolves its types, and runs Soufflé only on copies that pass. What the parser does not know is refused.

| Part | Admitted |
| --- | --- |
| Types | `symbol`; `number`, a signed 32-bit integer; subtypes, unions and records of them |
| Relations | `.decl` with the qualifiers `inline`, `no_inline`, `brie`, `btree`, `eqrel`, `overridable` |
| Structure | `.type`, `.comp`, `.init`, `.override`, `.plan`, `.once`; `.include` of `mara.dl` and the declared programs |
| Rules | Facts, rules, negation, recursion, comparison, `+`, `-`, `*`, the bitwise and logical operators, `range` |
| Text | `cat`, `strlen`, `substr`, `contains`, `to_string`, `to_number` |
| Aggregates | `count`, `sum`, `min`, `max` |

- A refusal is reported like a compile error, at the place the parser stopped ([REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)). [EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG) shows what each construct below did in Soufflé.
- Left out because it reaches outside: every input and output directive and qualifier, which Mara generates; `.functor`, `.pragma`, `.lattice`, `@` functors, `#` directives, `__FILE__`, `__INCL__`.
- Left out because its value depends on evaluation order: `autoinc()`, a bare `$`, `ord`, `choice-domain`, `recursive_iteration_cnt`, `.limitsize`.
- Left out because the native and the browser build differ, or Soufflé gets it wrong: `float` and `unsigned` types and literals, `mean`, `to_float`, `to_unsigned`, `as` casts, `match`, `/`, `%`, `^`, `magic`, subsumption, ADTs.
- A relation that crosses the boundary (a fact relation, a supplied input, a consumed output) has only `symbol` and `number` columns, or subtypes of them. The parser checks this on the resolved types.
- Time is bounded ([REQ-EVALUATION-TIME-LIMIT](rules-evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)).
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

A construct that the parser admits, or a memory fault in Soufflé, lets a program from an untrusted checkout read or write files or run code as the user who validates it. Five reviews each found ways past earlier, weaker checks ([EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).

---

## Open questions

- Whether the `souffle` child process also runs in an operating-system sandbox, or an accepted Decision accepts [RISK-SOUFFLE-REACHES-OUTSIDE](#01M7CYC90BZBBXGC0CZ66W8ZEV), and whether an evaluation gets limits on output size and memory besides time. The maintainer decides before the release.
