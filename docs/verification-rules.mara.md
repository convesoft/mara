# Rules verification

---

<a id="01M7HZYK9DYQDY8XFKWZES1ZB6"></a>

### Run project programs through every consumer

| Type | Verification |
| --- | --- |
| ID | VER-PROGRAM-CONSUMERS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](rules.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |

Rust integration tests run the real CLI on disposable projects of the `empty` template with a small hand-written schema, programs under `.mara/rules/` and their entries in the configuration ([DES-RULE-CONSUMERS](rules.mara.md#01M7BF6HE529B61Q2ZNXFQ1JXE)). The binary runs with an environment that holds only `HOME` and `TMPDIR`, both inside the test's temporary directory, and what mara-vt2ktq uses to locate the pinned `souffle`: no compiler is reachable. The other rules Verification items use this setup. Every consumed relation in them starts with the MID of the item its row is about, as DES-RULE-CONSUMERS requires; a row described without one is joined to an item of the project.

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
| A gate over four named items: one without a subject row; three with one, of which the first also has a condition row and an unavailable row, the second only an unavailable row and the third nothing else | Not applicable, failed, unavailable and passed, each listed with the rows that caused it; the gate failed ([DES-GATE-OUTCOME](rules.mara.md#01M7CT60KEEZRJCPJ0CPT7YT5B)) |
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
| Verifies | [REQ-PROJECT-AUTHORED-LOGIC](rules.mara.md#01M7BF6HDMR4G13G7VFDA770ZN) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), a program includes `mara.dl` and copies each of its relations into a consumed relation that starts with an item's MID; `query execute` returns them ([DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)). `item`, `title`, `revision` and `field` are copied with the same columns. `edge` is copied as source MID, relation, target MID, path and line; `folded` is joined to each title and field value it folds, as MID, original and folded text; `source_complete` and a supplied input are joined to every `item`, as MID and the relation's columns. The expectations compare the fact columns after these projections. The project has items of two types and statuses in two documents, a field, and relation rows in the metadata tables of two items.

| Case | Expectation |
| --- | --- |
| `item`, `title`, `field` and `edge` rows | One per item, title, field row and relation row, with the values that `get` and `relation list` give: schema names for type, status, key and relation, the item's anchor line, and for `edge` the path and line of its relation row |
| `revision` | One row per item, equal to the `revision` that `get` returns |
| `folded` of a title `ÉCOLE` written with a combining accent | `école`, with a precomposed `é`, on that item's row |
| A title and a field value holding a tab, a quote, a backslash and non-ASCII text | Returned as `get` gives them, each value one row |
| A supplied input value holding a newline and a tab ([VER-REQUEST-INPUTS](#01M7HZYKAH5ZEC7HMT3D0RYTP2)) | Returned unchanged, one row per item |
| `source_complete` without `--complete`; with `--complete` of an input the entry lists | No row; one row per item, naming that input |
| A rule that compares a `Mid` with an `Id` | A compile error ([VER-PROGRAM-FAULTS](verification-programs.mara.md#01M7HZYKB4R632Q63YWS8CY1FJ)) |
| From 0.4.0-alpha.6 (mara-xsm8w6), each case in the browser's Soufflé | The same rows |

---

<a id="01M7HZYKAH5ZEC7HMT3D0RYTP2"></a>

### Supply external records to a gate

| Type | Verification |
| --- | --- |
| ID | VER-REQUEST-INPUTS |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-EXTERNAL-RECORDS-AS-SUPPLIED](rules.mara.md#01M7BF6HDVZSZM8E9BAEF7NNQ0) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), a gate lists the inputs `candidate` (one symbol column) and `result` (two symbols and a number). A named item with a `fail` row in `result` is in its condition relation. A named item without a `result` row is in its unavailable relation, or in its condition relation when `source_complete("result")` holds ([DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).

| Case | Expectation |
| --- | --- |
| `--input candidate=abc` and `--input result=@results.json` | The program reads exactly those rows; the `fail` row fails its item as supplied |
| The item without a result, without `--complete result`; with it | Unavailable; failed |
| The same rows passed as rows through MCP | The CLI's result ([VER-CLI-MCP-PARITY](verification-hosts.mara.md#01M7F74J5Z4KX9F3HN78T1WB85)) |
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
| Verifies | [REQ-EXPLAIN-OUTCOMES](rules-evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE) |

In the setup of [VER-PROGRAM-CONSUMERS](#01M7HZYK9DYQDY8XFKWZES1ZB6), `query report` explains rows of invariants and gates ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)). Each leaf of a fact relation is compared with what `get` and `relation list` give for its item or relation row; a leaf of a supplied input names that input and holds the row's values as supplied ([REQ-EXPLAIN-OUTCOMES](rules-evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE)).

| Case | Method | Expectation |
| --- | --- | --- |
| A row of an invariant that joins `item`, `field` and `edge`, and a condition row of a gate that reads a supplied input | Test | A proof of that row whose leaves are input facts, each fact leaf at the path and line of its item or relation row, and the input's leaf with its name and the supplied row's values |
| Rows whose proofs pass a negated atom, a record, a component and an `eqrel` relation | Test | Explained |
| A row of an ancestry closure over a chain of 8 `Decomposes` links | Test | A proof whose leaves include the 8 `edge` rows, with no `subproof` reference left |
| A rule with a comparison of symbols, `x < "z"` | Test | Explained, with no constraint leaf |
| Rows whose value holds a newline from a supplied input, a quote or a backslash, or is a negative number | Test | Cannot explain; never a proof of another row |
| A row whose proof passes an aggregate | Test | Cannot explain |
| Each case | Test | `project validate` and the gate return the same result as without the report |
| The check that a returned proof is for the requested row, and the handling of a reply that is not JSON and of an aborted provenance run | Inspection | Review of mara-bx82k0 finds each; no admitted program is known to reach them ([EVD-SOUFFLE-PROGRAM-REACH](rules-evaluation.mara.md#01M7CMW70TAFCJKPAX3VY94QMG)) |
| From 0.4.0-alpha.6 (mara-xsm8w6), the test cases in the browser | Test | The same proofs |
