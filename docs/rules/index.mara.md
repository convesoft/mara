# Rules

| Page | Holds |
| --- | --- |
| [Rule evaluation](evaluation.mara.md) | Running Soufflé, limits, faults, explanations, Soufflé evidence |
| [Rules decisions](decisions.mara.md) | Soufflé, program profile, consumers, fact bridge, gates |
| [Rules verification](verification.mara.md) | Checks of program consumers, facts, request inputs, explanations, program faults, the time limit, the admitted language and reaching outside |

## Requirements

---

<a id="01M7BF6HDMR4G13G7VFDA770ZN"></a>

### Projects write their own logic

| Type | Requirement |
| --- | --- |
| ID | REQ-PROJECT-AUTHORED-LOGIC |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](../product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

A project defines invariants, named queries, gates and views as Soufflé Datalog programs, in the admitted language ([DES-PROGRAM-PROFILE](#01M7CMW7051QVPFDWQBMB1A6K6)), over the facts Mara supplies. Adding or changing a program needs no compiler and no change to Mara.

---

<a id="01M7BF6HDVZSZM8E9BAEF7NNQ0"></a>

### External records are evaluated as supplied

| Type | Requirement |
| --- | --- |
| ID | REQ-EXTERNAL-RECORDS-AS-SUPPLIED |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](../product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

Mara evaluates externally supplied records, such as verification results, as given: it never runs tests or vouches for their truth. A record counts as missing only when its source is marked complete; otherwise the outcome is unavailable.

---

<a id="01M7CMW6ZTAJHSF5BR9SF9KR86"></a>

### Programs only compute

| Type | Requirement |
| --- | --- |
| ID | REQ-PROGRAMS-ONLY-COMPUTE |
| Status | Accepted |
| Satisfies | [NEED-PROJECT-LOGIC](../product.mara.md#01M7BF6H74D5GX045CWVY9106D) |

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

| Consumer | Configuration entry | Effect |
| --- | --- | --- |
| Query | Program and relation | Returns the output rows |
| View | Program and relation | Renders the same rows |
| Invariant | Program, relation, severity and message | Evaluated by validation; an error-severity failure makes documentation invalid |
| Gate | Program; subject, condition and unavailable relations | Judges the items a request names ([DES-GATE-OUTCOME](#01M7CT60KEEZRJCPJ0CPT7YT5B)); never changes documentation validity |

- Every entry has a name and a description, and lists the inputs a request must supply ([DES-REQUEST-INPUTS](#01M7CT60KMGA2M9WMQ8RRJREPG)). `query list` shows them all.
- `[rules] programs` lists the patterns that declare program files ([DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)); the line of the first pattern that matches a file declares it. `time_limit_seconds`, optional, from 1 to 3600, sets the time limit ([REQ-EVALUATION-TIME-LIMIT](evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)). An entry table without `[rules]` is `missing-key`.
- A name starts with a lowercase ASCII letter and holds lowercase ASCII letters, digits, `-` and `_`; it is unique across the four tables (`duplicate-name`). An entry whose name is invalid or taken is still checked, then left out. An entry's `program` is a file that a pattern declares.
- `inputs` is optional and empty by default. An invariant has none, since validation supplies none, and its `description` defaults to its `message`. `severity` is `error` or `warning`. A gate's `subjects` and `conditions` name at least one relation; `unavailable` is optional.
- Against its admitted program, an entry naming a relation or input that the program does not declare is `undeclared-relation`; a consumed relation whose first column is no symbol, or an input whose facts Mara supplies, is `invalid-relation`; both at the configuration line that names it. Such an entry does not run. Since they are found only against the admitted program, they are reported by validation and fail the operations that evaluate the entry, as a compile error does, and do not refuse the project to operations that evaluate nothing ([DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)).
- A request addresses a consumer only by its name. A relation without an entry is not reachable.
- The first column of a consumed relation is the MID of the item its row is about. It gives a failure its path and line, and a query or view request that names items gets only their rows.
- A row of an invariant relation is one failure. A row whose first column is no item's MID is reported at the configuration line that names the relation.
- `query report` explains one row of an invariant or gate outcome ([REQ-EXPLAIN-OUTCOMES](evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE)).
- An empty result has no pass or fail meaning of its own; the consumer defines it.
- A gate reports conditions, not permission to release, deploy or act.

```toml
[rules]
programs = [".mara/rules/*.dl"]

[queries.requirement-basis]
program = ".mara/rules/engineering.dl"
relation = "requirement_basis"
description = "Basis candidates of a requirement, with their status"

[invariants.requirement-verified]
program = ".mara/rules/engineering.dl"
relation = "unverified_requirement"
severity = "error"
message = "Accepted requirement has no accepted verification"

[gates.verified]
program = ".mara/rules/gates.dl"
description = "Named requirements are implemented and pass every accepted verification"
subjects = ["requirement"]
conditions = ["not_accepted", "not_implemented", "verification_not_passed"]
unavailable = ["code_unavailable", "result_unavailable"]
inputs = ["candidate", "verification_result"]
```

---

<a id="01M7CT60KEEZRJCPJ0CPT7YT5B"></a>

### Gate scope and outcome

| Type | Design |
| --- | --- |
| ID | DES-GATE-OUTCOME |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](#01M7BF6HDMR4G13G7VFDA770ZN) |
| Satisfies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

A gate request names the items to judge; an empty scope is refused. Mara judges each named item by the rows of the gate's relations whose first column is that item:

| Rows for the item | Item outcome |
| --- | --- |
| None in a subject relation | Not applicable: the gate cannot judge this item |
| Any in a condition relation | Failed |
| Otherwise, any in an unavailable relation | Unavailable |
| Otherwise | Passed |

- The gate passes only when every named item passed. It is failed when an item failed or is not applicable, and unavailable otherwise.
- The result lists each item's outcome with the rows that caused it.

---

<a id="01M7CT60KMGA2M9WMQ8RRJREPG"></a>

### Inputs a request supplies

| Type | Design |
| --- | --- |
| ID | DES-REQUEST-INPUTS |
| Status | Accepted |
| Satisfies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

- An input is a relation that a program declares and its consumer lists as an input, such as a candidate or verification results. `mara.dl` declares none ([DES-FACT-BRIDGE](#01M7BF6HEHHJ7XZM8HDFXK115J)).
- A request supplies each input its consumer lists: one value inline (`--input candidate=abc123`), or a JSON file holding a list of rows in the relation's column order (`--input verification_result=@results.json`). MCP's `input` is an object from each input's name to one value as a string or to its list of rows; a string there is always one value, never a file.
- An inline value is one row of one column, written as a decimal integer for a number column. A file path is relative to the directory the command runs in.
- A request that omits a listed input, supplies one its consumer does not list or supplies one twice, or marks one complete that its consumer does not list, is refused and names it.
- Mara checks every supplied row before the run: the number of columns, a string for a symbol column, and an integer from −2,147,483,648 to 2,147,483,647 for a number column. A wrong row refuses the request and names its row and column, because Soufflé would read it as 0 or an empty string.
- `--complete <input>` states that the input holds every record of its source; Mara then adds `source_complete("<input>")`. Mara itself adds `source_complete("code")` when it has read all code ([DES-CODE-READERS](../code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)).
- Inputs are never stored. They are part of the evaluation key ([DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).

---

<a id="01M7JT87HXDH2D4V1FNM6VZT7N"></a>

### Query requests and results

| Type | Design |
| --- | --- |
| ID | DES-QUERY-RESULTS |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](#01M7BF6HDMR4G13G7VFDA770ZN) |
| Satisfies | [REQ-CLI-MCP-PARITY](../hosts/index.mara.md#01M7BF6H00N50GBQGCYF4RSR1Z) |

| Operation | Takes | Result |
| --- | --- | --- |
| `query list` | `--limit`, `--cursor` | `entries`: `name`, `kind` (`query`, `view`, `invariant`, `gate`), `description`, `inputs` (names), by table, then by name |
| `query execute <name>`, `query view <name>` | `--id`, `--input`, `--complete`, `--limit`, `--cursor` | `query`: `name`, `kind`, `relation`, `columns` (`name`, `type` `symbol` or `number`); `rows`: `item`, the node of the item whose MID the first column holds or `null`, and `values`, in the order of their values |
| `query gate <name>` | `--id` at least once, `--input`, `--complete`, `--limit`, `--cursor` | `gate`: `name`, `outcome`; `items`: `item`, `outcome` (`passed`, `failed`, `unavailable`, `not-applicable`), `rows` (`relation`, `role` `subject`, `condition` or `unavailable`, `values`) |
| `query report <name>` | `--id` once, `--input`, `--complete`, `--limit`, `--cursor` | `report`: `name`, `kind` (`invariant`, `gate`), `item`, `outcome`; `rows`: `relation`, `role` for a gate, `values`, and either `proof` or `unexplained`, the reason there is none |

- `--id` takes an ID or MID, repeatable: a query or view returns only the rows about those items, a gate judges each once in the order given. A gate request without `--id`, and a report request without one `--id`, are rejected as `invalid-argument` before anything runs.
- `query list` reads the configuration and the schema only and runs nothing. The others evaluate the entry's program once with the snapshot's facts; every other entry of that program shapes the same run, and an input that only another entry lists is read empty, as validation reads it.
- A request is refused before any run for a name that no entry of the operation's kind has (`unknown-entry`, naming the operation that runs it), an unknown or ambiguous `--id`, or an input refusal of [DES-REQUEST-INPUTS](#01M7CT60KMGA2M9WMQ8RRJREPG) (`missing-input`, `unlisted-input`, `repeated-input`, `invalid-input`).
- A query or view whose program gives no rows, a missing or refused program, an entry that does not fit it, a compile error or a fault, fails with `program-failed`, those problems as its diagnostics. A gate is then `unavailable` with every item, no rows, and those problems first in its `diagnostics`, each on every page whatever its size, before the inventory's, which keep their budget.
- A project whose configuration or schema has an error refuses every `query` request as `invalid-project` before any outcome, as every operation but validation ([DES-PROJECT-CONFIGURATION](../project.mara.md#01M7BF6H9RDYSET9QTAKJWTX27)); `unavailable` is the outcome of a gate whose evaluation failed.
- Results page as reads do ([DES-READ-RESULTS](../retrieval/index.mara.md#01M7DMT8GAJCAFTW0Y2939JFF1)). The result key covers the evaluation key, the operation, the entry and the named items' MIDs.
- Both forms of `query execute` and `query view` hold the same JSON; the human form of a view is a Markdown table under the column names, that of a query one line per row. A gate exits with 0 only when it passed.
- A file that `--input` names counts as a file read, not hashed: it is no input of the project.
- `query report` takes an invariant or a gate and the item `--id` names. Its rows are those that make the entry's outcome for the item: an invariant's rows about the item, the outcome `failed` when there is one and `passed` otherwise; a gate's rows that `query gate` lists for the item, with the item's outcome there. Its one run is in provenance mode ([DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)): its outputs give the outcome and the rows, and the explanations never change them. A fault leaves the report `unavailable` with no rows and its problems in `diagnostics`, as a gate's. It exits with 0 only when each of its rows is explained.
- A `proof` is a tree of steps: a derived row, `relation`, `atom` (the row as Soufflé prints it), `rule` (the rule that derived it, on one line) and `children`; a fact, `relation`, `values`, `path` and `line`, of a document or of a program that states it; a row of a supplied input, `input` and `values`; or a negated literal, `absent`. The human form prints one step per line, each indented under the step it serves.

---

<a id="01M7BF6HEHHJ7XZM8HDFXK115J"></a>

### Fact bridge

| Type | Design |
| --- | --- |
| ID | DES-FACT-BRIDGE |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-AUTHORED-LOGIC](#01M7BF6HDMR4G13G7VFDA770ZN) |
| Satisfies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

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

- `mara.dl` only declares the relations. For each run Mara writes their facts, one JSON file per relation and once per document snapshot, and generates the directives that read them ([DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).
- Types, statuses, field keys, enum values and relation names are schema names (`requirement`, `derives_from`), never labels.
- The distinct subtypes `Mid`, `Id` and `Path` make Soufflé reject rules that mix identities.
- `revision` is the hash of the item's region. `get` returns the same value, so whoever records a result can record the definition it belongs to.
- `folded` gives the NFC, case-folded form of every title and field value, as search normalizes text. Soufflé has no case folding.
- `item` holds an item's first `Type`, `ID` and `Status` values, an absent one as an empty symbol, and its anchor's line.
- `field` holds each row whose key the schema declares as a field of the item's type: an enum value as its schema name, a text value as written.
- `edge` holds the relation rows whose value links to a MID, at the line of that metadata row. Mentions are no edges.
- `folded` lowercases as search does, then applies NFC.
- The engine derives the rows and their JSON text once per snapshot and keeps them with it; the host writes that text into each run's private directory, one `<relation>.json` per relation.
- `source_complete` names a source whose records are all present ([DES-REQUEST-INPUTS](#01M7CT60KMGA2M9WMQ8RRJREPG)).
- Code links ([DES-CODE-MARKERS](../code/index.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)) become a further input relation.
- The file's format version starts at 1 and changes only with a released contract change, never for internal iterations.

---

<a id="01M7CMW7051QVPFDWQBMB1A6K6"></a>

### Admitted program profile

| Type | Design |
| --- | --- |
| ID | DES-PROGRAM-PROFILE |
| Status | Accepted |
| Satisfies | [REQ-PROGRAMS-ONLY-COMPUTE](#01M7CMW6ZTAJHSF5BR9SF9KR86) |
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

- A refusal is reported like a compile error, at the place the parser stopped ([REQ-PROGRAM-FAULTS-REPORTED](evaluation.mara.md#01M7BF6HCZ88PSSQTFKXE8KJ81)). [EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG) shows what each construct below did in Soufflé.
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
- Time is bounded ([REQ-EVALUATION-TIME-LIMIT](evaluation.mara.md#01M7CMW70G711Z67QEWW5Z0MK0)).
- Soufflé runs with library loading off and never in the project's directory ([DES-RULE-EXECUTION](evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).

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
| Threatens | [REQ-PROGRAMS-ONLY-COMPUTE](#01M7CMW6ZTAJHSF5BR9SF9KR86) |

A construct that the parser admits, or a memory fault in Soufflé, lets a program from an untrusted checkout read or write files or run code as the user who validates it. Five reviews each found ways past earlier, weaker checks ([EVD-SOUFFLE-PROGRAM-REACH](evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)). On Linux the sandbox ([DES-SOUFFLE-SANDBOX](evaluation.mara.md#01M7K4XGJ4TSYM1E64C327HXJQ)) contains such a program; on macOS nothing does yet.
