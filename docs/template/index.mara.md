# Engineering template: knowledge rules

| Page | Holds |
| --- | --- |
| [Readiness](readiness.mara.md) | Queries, result inputs, gates, views |
| [Engineering vocabulary](vocabulary.mara.md) | Types and relations of the `engineering` template |

## Requirements

---

<a id="01M7CT60KVJ75RV9GPP3SEF0WX"></a>

### Accepted knowledge obeys its acceptance rules

| Type | Requirement |
| --- | --- |
| ID | REQ-TEMPLATE-ACCEPTANCE-RULES |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](../product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

In a project that uses the `engineering` template, an accepted item that breaks its acceptance or lifecycle rule is a validation error. A draft item never is. No rule requires a design, an implementation, a result or a validation.

---

## Design

---

<a id="01M7CT60M2EQ33WTZFNG600D4H"></a>

### Acceptance rules

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-ACCEPTANCE |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-ACCEPTANCE-RULES](#01M7CT60KVJ75RV9GPP3SEF0WX) |

Each rule is an invariant of error severity over accepted items.

| Accepted type | Rule |
| --- | --- |
| Requirement | Has an accepted basis: it `Satisfies` a Need, `Decomposes` or `Derives from` a Requirement, or a Decision `Justifies` it. An accepted Verification `Verifies` it |
| Design | Has an accepted basis: it `Satisfies` a Requirement, `Contributes to` a Goal or `Decomposes` a Design. A justifying Decision is not a basis |
| Risk | Has exactly one `Likelihood` and one `Consequence`, and `Threatens` an accepted Goal, Need or Requirement |
| Verification | Has a method and `Verifies` an accepted Requirement |
| Validation | Has a method and `Validates` an accepted Need or Goal |
| Term | No title or alias of it equals, after folding ([DES-FACT-BRIDGE](../rules/index.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)), a title or alias of another accepted Term. No alias is empty or equals its own title |
| Goal, Need, Scenario, Decision, Evidence | None |

- No rule demands a chain: a Requirement may be implemented without a Design, and a Design may contribute straight to a Goal.
- Status records authority only. Implemented, verified, validated, ready and blocked are never a status or a field.

---

<a id="01M7CT60M96Q91T5KXNQ1G58PP"></a>

### Relation lifecycle

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-RELATION-LIFECYCLE |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-ACCEPTANCE-RULES](#01M7CT60KVJ75RV9GPP3SEF0WX) |

Invariants of error severity judge each relation by its class.

| Class | Relations | Rule |
| --- | --- | --- |
| Strict | Satisfies, Contributes to, Decomposes, Derives from, Threatens, Mitigates, Accepts, Verifies, Validates, Exercises | From an accepted source, the target is accepted |
| Developmental | Justifies, Supports | None: the target may be draft, accepted or retired |
| Replacement | Supersedes | From an accepted or retired source, the target is retired |

`C supersedes B supersedes A` therefore stays valid after B retires.

---

<a id="01M7CT60MFPF2Y871JJDZP5K0T"></a>

### Risk level and blocking risks

| Type | Design |
| --- | --- |
| ID | DES-TEMPLATE-RISK-LEVEL |
| Status | Accepted |
| Satisfies | [REQ-TEMPLATE-GATES-PROVE-READINESS](readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

A risk's level comes from its `Likelihood` (rows) and `Consequence` (columns).

| | Minimal | Minor | Moderate | Major | Severe |
| --- | --- | --- | --- | --- | --- |
| Rare | LOW | LOW | LOW | MEDIUM | MEDIUM |
| Unlikely | LOW | LOW | MEDIUM | MEDIUM | HIGH |
| Possible | LOW | MEDIUM | MEDIUM | HIGH | HIGH |
| Likely | MEDIUM | MEDIUM | HIGH | HIGH | HIGH |
| Almost-certain | MEDIUM | HIGH | HIGH | HIGH | HIGH |

- The matrix ships as 25 facts in `.mara/rules/risk-matrix.dl`. A project tailors it by editing them.
- A risk that lacks a field, or whose pair has no cell, has no level. Whatever depends on that level is unavailable, never LOW.
- A `Mitigates` link does not lower a level. Only a changed `Likelihood` or `Consequence` does.
- A risk is relevant to a requirement when it `Threatens` the requirement, one of its ancestors through `Decomposes` and `Derives from`, a Need one of those satisfies, or a Goal such a Need contributes to. Sibling requirements never enter.
- A blocking risk is an accepted, relevant risk of level HIGH that no accepted Decision `Accepts`.

---

## Decisions

---

<a id="01M7CT60NYQQPN6DG5J6WJCMSA"></a>

### Keep validity, completeness and readiness apart

| Type | Decision |
| --- | --- |
| ID | ADR-VALIDITY-NOT-READINESS |
| Status | Accepted |
| Justifies | [REQ-TEMPLATE-ACCEPTANCE-RULES](../template/index.mara.md#01M7CT60KVJ75RV9GPP3SEF0WX) |
| Justifies | [REQ-TEMPLATE-GATES-PROVE-READINESS](../template/readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

- Chosen: invariants judge only whether accepted knowledge is authoritative, gates judge named items for a candidate, and missing downstream work is neither.
- Rejected: one validity that also covers implementation and results. It turns the whole development lifecycle into document validity.
- Rejected: implemented, verified or ready as a status or a field. They depend on the candidate and go stale.
- Rejected: a mandatory chain from goal to code. A small change needs only a requirement and its verification.

---

<a id="01M7CT60P4PXK6DMZZ28QQBBNY"></a>

### Ship a NASA-informed default risk matrix

| Type | Decision |
| --- | --- |
| ID | ADR-NASA-INFORMED-RISK-MATRIX |
| Status | Accepted |
| Justifies | [DES-TEMPLATE-RISK-LEVEL](../template/index.mara.md#01M7CT60MFPF2Y871JJDZP5K0T) |

- Chosen: a default of 25 cells informed by NASA/SP-2011-3422 §4.3.2.6, which a project tailors. It is neither NASA's own matrix nor a compliance claim.
- Rejected: no default. Every result that depends on a risk level would stay unavailable until a project writes 25 cells.
- Rejected: likelihood times consequence as a score. A multiplication is not an adopted policy.

---

<a id="01M7CT60PAQ0JPNNETP6TH90DD"></a>

### Keep supersession links of retired items valid

| Type | Decision |
| --- | --- |
| ID | ADR-HISTORICAL-SUPERSESSION |
| Status | Accepted |
| Justifies | [DES-TEMPLATE-RELATION-LIFECYCLE](../template/index.mara.md#01M7CT60M96Q91T5KXNQ1G58PP) |

- Chosen: a `Supersedes` link stays valid after the replacing item retires.
- Rejected: requiring an accepted source for every `Supersedes` link. A second replacement would invalidate the lineage that the first one records.

---

## Verification

---

<a id="01M7CT60NQK9TAH7JD15HCR2XE"></a>

### Exercise the engineering template's acceptance cases

| Type | Verification |
| --- | --- |
| ID | VER-TEMPLATE-CASES |
| Status | Accepted |
| Methods | Test |
| Verifies | [REQ-TEMPLATE-ACCEPTANCE-RULES](#01M7CT60KVJ75RV9GPP3SEF0WX) |
| Verifies | [REQ-TEMPLATE-GATES-PROVE-READINESS](readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

Rust integration tests run the real CLI on disposable projects initialized from `engineering`.

| Case | Expectation |
| --- | --- |
| Accepted requirement with only a draft basis, or only a draft verification | Validation error; the same items as drafts are valid |
| Requirement with two accepted verifications, one passing and the other failing or absent from a complete input | `verified` fails |
| Result for another candidate, or for an older revision of the verification or of the requirement | Does not count: `verification-status` is `pending` with a complete input and `unavailable` with an incomplete one; `verified` does not pass |
| Validation result for another context, or for an older revision of the validation or of its need or goal | Does not count: `validation-status` is `pending` with a complete input and `unavailable` with an incomplete one; `release-ready` does not pass |
| For one verification and requirement, and for one validation and need, counting results `pass` and `fail`, `pass` and `error`, and `pass` and `inconclusive` | `verification-status` and `validation-status` give the worse result; `verified` and `release-ready` fail |
| Risk on a parent requirement, on its need or on that need's goal | Relevant to the child requirement; a sibling requirement is not judged |
| HIGH risk with a mitigation | Blocks until it is reassessed or an accepted decision accepts it |
| Named validation whose result is absent from a complete input, or is not a pass | `release-ready` fails; a validation that is not named does not block |
| Validation of two needs with a pass for one and, for the other, a failing result or none in a complete input | `release-ready` fails |
| A failure and an unavailable fact on the same item | Failed |
| Item whose only missing fact is a risk level, an implementation link while code is unread, or a result absent from an incomplete input | Unavailable, never passed |
| Accepted requirement without an accepted verification | `verified` fails |
| A failing row of each template invariant and gate | Has an explanation |
| `C supersedes B supersedes A` after B retires | Valid |
| Requirement implemented only through a design, from 0.4.0-alpha.5 (mara-yhxq5x) | Counts when the design is accepted, not when it is draft |
| From 0.4.0-alpha.5, a requirement that an `implements` marker names, with an accepted Verification whose result passes and a named Validation of its Need that passes | `implemented` has the marker's row; `verified` and `release-ready` pass |
| From 0.4.0-alpha.5, markers naming a Design and a Verification | `implemented` has a row for each |
| From 0.4.0-alpha.5, a requirement that no marker names, every code file read; the same in a project that declares no code language | `verified` and `release-ready` fail ([DES-CODE-READERS](../code/index.mara.md#01M7CQ57TTB40GWX8YHKB259DK)) |
| From 0.4.0-alpha.5, a requirement that no marker names while a code file cannot be read | `verified` and `release-ready` unavailable |
| A view and a gate over the same items | Report the same facts |
| One project per rule of [DES-TEMPLATE-ACCEPTANCE](#01M7CT60M2EQ33WTZFNG600D4H) and of [DES-TEMPLATE-RELATION-LIFECYCLE](#01M7CT60M96Q91T5KXNQ1G58PP), with an accepted item that breaks it | A validation error of that rule's invariant at the item; valid when the item is a draft |
| A risk in each of the 25 cells of [DES-TEMPLATE-RISK-LEVEL](#01M7CT60MFPF2Y871JJDZP5K0T) | `risk-level` gives that cell's level |
| `requirement-basis` of a requirement with a draft and an accepted basis | Both rows, with their statuses |
| Each view of [DES-TEMPLATE-VIEWS](readiness.mara.md#01M7CT60NHPG3GY0V1F0MZ5QF0) for a requirement without an accepted verification, one whose result is absent from an incomplete input, and one with a failing result | A row for each; the cells for missing, unavailable and failed differ |
