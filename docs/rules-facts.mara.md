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

Mara ships a versioned declarations file, `mara.dl`, that starts with `.once`; programs include it with `.include "mara.dl"`. It declares only relations that every vocabulary has. Version 1:

```souffle
.once
// mara.dl format version 1 (DES-FACT-BRIDGE). Mara supplies these relations' facts.

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
- `item` holds an item's first `Type`, `ID` and `Status` values, an absent one as an empty symbol, and its anchor's line.
- `field` holds each row whose key the schema declares as a field of the item's type: an enum value as its schema name, a text value as written.
- `edge` holds the relation rows whose value links to a MID, at the line of that metadata row. Mentions are no edges.
- `folded` lowercases as search does, then applies NFC.
- The engine derives the rows and their JSON text once per snapshot and keeps them with it; the host writes that text into each run's private directory, one `<relation>.json` per relation.
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

- A refusal is reported like a compile error, at the place the parser stopped ([REQ-PROGRAM-FAULTS-REPORTED](rules-evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)). [EVD-SOUFFLE-PROGRAM-REACH](rules-evidence.mara.md#01M7CMW70TAFCJKPAX3VY94QMG) shows what each construct below did in Soufflé.
- Left out because it reaches outside: every input and output directive and qualifier, which Mara generates; `.functor`, `.pragma`, `.lattice`, `@` functors, `#` directives, `__FILE__`, `__INCL__`.
- Left out because its value depends on evaluation order: `autoinc()`, a bare `$`, `ord`, `choice-domain`, `recursive_iteration_cnt`, `.limitsize`.
- Left out because the native and the browser build differ, or Soufflé gets it wrong: `float` and `unsigned` types and literals, `mean`, `to_float`, `to_unsigned`, `as` casts, `match`, `/`, `%`, `^`, `magic`, subsumption, ADTs.
- A relation that crosses the boundary (a fact relation, a supplied input, a consumed output) has only `symbol` and `number` columns, or subtypes of them, and is not `inline`. The parser checks this on the resolved types.

The parser reads text as Soufflé 2.5 does with `--no-preprocessor` and refuses text that the two could read differently:

| Part | Admitted |
| --- | --- |
| Text | UTF-8; outside strings and comments only ASCII; no control character but tab, line feed and carriage return |
| Comments | `//` up to the line feed, `/* */` without nesting; `#` only in comments and strings |
| Doc comments | `///` and `/** */` only right before a declaration, a fact or a rule; no `//!` or `/*!` |
| Strings | On one line; the escapes `\"`, `\\`, `\'`, `\n`, `\t`, `\r` |
| Integers | Decimal, `0x` and `0b`, from −2^31 to 2^31−1; 2^31 only right after a unary `-` |
| Names | No Soufflé keyword or built-in type name, no `?`, no leading `__` |
| Directives | Each but `.once` followed by a space or a tab |
| `.include` | Only between items outside components, its quoted path after spaces or tabs: relative, without `.` or `..` parts, resolved against the including file's directory to a declared program, else `mara.dl` to Mara's declarations. A file included twice starts with `.once`; `.once` comes first in a file. A declared program at the project path `mara.dl` is refused |
| Aggregates | `count :`, or `sum`, `min` or `max` of one value; no `;` at any depth of the body; `min` and `max` only of values that the aggregate's own atoms or comparisons show to be numbers, also through groups without `;` and through record fields |
| Comparisons | `<`, `<=`, `>` and `>=` not on records |
| `.plan` | Only after a recursive rule with one head, no `;` and no parentheses around terms; versions below its number of recursive atoms, each order a permutation of its atoms |
| Components | A relation or type name resolves in its instance, then the enclosing instances, then the program; a type parameter only in its own component's body; a type argument is one name, without `.` |
| Size | The parser descends at most 100 levels, each term, value, operator, group, record, call, aggregate, negation and component definition taking one or more; component instances and inheritance together nest at most 100 deep; at most 100,000 declarations, clauses, instances and inherited bodies once components are expanded. A program past a limit is refused, naming it |
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

A construct that the parser admits, or a memory fault in Soufflé, lets a program from an untrusted checkout read or write files or run code as the user who validates it. Five reviews each found ways past earlier, weaker checks ([EVD-SOUFFLE-PROGRAM-REACH](rules-evidence.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)).

---

## Open questions

- Whether the `souffle` child process also runs in an operating-system sandbox, or an accepted Decision accepts [RISK-SOUFFLE-REACHES-OUTSIDE](#01M7CYC90BZBBXGC0CZ66W8ZEV). The maintainer decides before the release.
