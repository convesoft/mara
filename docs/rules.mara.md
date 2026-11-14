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

| Consumer | Configuration entry | Effect |
| --- | --- | --- |
| Query | Program and relation | Returns the output rows |
| View | Program and relation | Renders the same rows |
| Invariant | Program, relation, severity and message | Evaluated by validation; an error-severity failure makes documentation invalid |
| Gate | Program; subject, condition and unavailable relations | Judges the items a request names ([DES-GATE-OUTCOME](#01M7CT60KEEZRJCPJ0CPT7YT5B)); never changes documentation validity |

- Every entry has a name and a description, and lists the inputs a request must supply ([DES-REQUEST-INPUTS](#01M7CT60KMGA2M9WMQ8RRJREPG)). `query list` shows them all.
- A request addresses a consumer only by its name. A relation without an entry is not reachable.
- The first column of a consumed relation is the MID of the item its row is about. It gives a failure its path and line, and a query or view request that names items gets only their rows.
- A row of an invariant relation is one failure.
- `query report` explains one row of an invariant or gate outcome ([REQ-EXPLAIN-OUTCOMES](rules-evaluation.mara.md#01M7BF6HCQX66B3MYRH5BZM6BE)).
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

- An input is a relation that a program declares and its consumer lists as an input, such as a candidate or verification results. `mara.dl` declares none ([DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)).
- A request supplies each input its consumer lists: one value inline (`--input candidate=abc123`), or a JSON file holding a list of rows in the relation's column order (`--input verification_result=@results.json`). MCP passes the rows.
- A request that omits a listed input is refused and names it.
- Mara checks every supplied row before the run: the number of columns, a string for a symbol column, and an integer from −2,147,483,648 to 2,147,483,647 for a number column. A wrong row refuses the request and names its row and column, because Soufflé would read it as 0 or an empty string.
- `--complete <input>` states that the input holds every record of its source; Mara then adds `source_complete("<input>")`. Mara itself adds `source_complete("code")` when it has read all code ([DES-CODE-READERS](code.mara.md#01M7CQ57TTB40GWX8YHKB259DK)).
- Inputs are never stored. They are part of the evaluation key ([DES-RULE-EXECUTION](rules-evaluation.mara.md#01M7BF6HD6K4B2K5Q7FJVGCDNH)).
