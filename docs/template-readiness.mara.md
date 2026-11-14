# Engineering template: readiness

## Requirements

---

<a id="01M7CT60MNF0XPXR9NB6X74F3B"></a>

### Gates pass only on proven readiness

| Type | Requirement |
| --- | --- |
| ID | REQ-TEMPLATE-GATES-PROVE-READINESS |
| Status | Accepted |

A gate of the `engineering` template passes only when every named item is proven ready for the supplied candidate. A failed, missing, stale or unavailable fact never yields a pass. No gate result changes validity.

---

## Design

---

<a id="01M7CT60MW58QYX605NJNARD6H"></a>

### Queries

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-QUERIES |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-GATES-PROVE-READINESS](#01M7CT60MNF0XPXR9NB6X74F3B) |
| Satisfies | [REQ-TEMPLATE-ACCEPTANCE-RULES](template.mara.md#01M7CT60KVJ75RV9GPP3SEF0WX) |

| Query | Rows, for the item in the first column |
| --- | --- |
| `requirement-basis` | Requirement: each basis candidate with its type, relation and status |
| `design-basis` | Design: each basis candidate with its relation and status |
| `term-collisions` | Term: each other Term that shares a folded title or alias, with its status, the value and the kind of match |
| `relation-lifecycle` | Source item: relation, target, both statuses and the class |
| `implemented` | Requirement, Design or Verification: each code link that implements it, directly or, for a Requirement, through an accepted Design that satisfies it |
| `risk-level` | Risk: likelihood, consequence and level |
| `risk-scope` | Requirement: every node through which a risk can be relevant to it ([DES-TEMPLATE-RISK-LEVEL](template.mara.md#01M7CT60MFPF2Y871JJDZP5K0T)) |
| `relevant-risks` | Requirement: each relevant risk with its status and the node it threatens |
| `verification-status` | Requirement: each Verification with its status and its result for the candidate |
| `validation-status` | Need or Goal: each Validation with its status and its result in its context, and one summary row |

- A query keeps items of every status. Invariants, gates and views decide which statuses count.
- Invariants, gates and views are written over these relations, so they cannot disagree.
- A lookup along one relation gets no query; `relation list` answers it.

---

<a id="01M7CT60N3AT6378YRQ62A5NM3"></a>

### Results a request supplies

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-INPUTS |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-GATES-PROVE-READINESS](#01M7CT60MNF0XPXR9NB6X74F3B) |

| Input | Columns |
| --- | --- |
| `candidate` | Candidate identity; one row |
| `verification_result` | Verification ID, candidate, revision of the Verification, result |
| `validation_context` | Validation ID, context; one row for each Validation the request names |
| `validation_result` | Validation ID, context, revision of the Validation, result |

- A result is `pass`, `fail`, `inconclusive` or `error`. A context is opaque text, such as a deployment or an observation window.
- A result counts only when its candidate or context is the requested one and its revision is the item's current revision ([DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)).
- When several results count, the worst wins: `fail`, `error`, `inconclusive`, `pass`.
- Without a counting result the status is `pending` when the input is complete and `unavailable` when it is not ([DES-REQUEST-INPUTS](rules.mara.md#01M7CT60KMGA2M9WMQ8RRJREPG)).
- The summary row of `validation-status` is the worst status of the target's accepted Validations, with `pending` and `unavailable` ranked between `inconclusive` and `pass`. A target without an accepted Validation is not validated.

---

<a id="01M7CT60NAX3YP36Y6M5GPT7X2"></a>

### Gates

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-GATES |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-GATES-PROVE-READINESS](#01M7CT60MNF0XPXR9NB6X74F3B) |

| Gate | Subjects | An item fails when |
| --- | --- | --- |
| `implementation-ready` | Requirements | It is not accepted, it breaks an acceptance or lifecycle rule, or a blocking risk is relevant to it ([DES-TEMPLATE-RISK-LEVEL](template.mara.md#01M7CT60MFPF2Y871JJDZP5K0T)) |
| `verified` | Requirements | It is not accepted, `implemented` has no row for it, or one of its accepted Verifications has no counting `pass` for the candidate |
| `release-ready` | Requirements and Validations | A Requirement fails either gate above. A Validation is not accepted, has no context, or has no counting `pass` in it |

- An item is unavailable when a relevant accepted risk has no level, when code was not read completely, or when a result is absent from an incomplete input.
- Every accepted Verification of a requirement must pass; one pass does not cover another.
- No gate requires a Design, and none adds decomposed, parent or sibling requirements to the named items.
- `release-ready` judges only the Validations a request names. Naming none claims no validation.
- Mara computes each outcome from these relations ([DES-GATE-OUTCOME](rules.mara.md#01M7CT60KEEZRJCPJ0CPT7YT5B)).

---

<a id="01M7CT60NHPG3GY0V1F0MZ5QF0"></a>

### Views

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-VIEWS |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-INIT](project.mara.md#01M7BF6H8Z02SNKXD2XPGVD87M) |

| View | One row per | Columns |
| --- | --- | --- |
| `delivery-readiness` | Requirement | Knowledge validity, implementation, verification for the candidate, blocking risks |
| `requirement-verification` | Requirement and Verification | Implementation path, code links, the Verification and its status, its result for the candidate |
| `risk-treatment` | Requirement and relevant risk | Threatened node, assessment and level, mitigations, accepting Decision, blocking or not |

- A row stays when a fact is absent, and its cell says so. Missing, unavailable and failed stay distinct.
- A view defines no meaning of its own: it renders relations that the queries and gates define.
