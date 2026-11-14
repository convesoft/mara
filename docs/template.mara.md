# Engineering template: knowledge rules

## Requirements

---

<a id="01M7CT60KVJ75RV9GPP3SEF0WX"></a>

### Accepted knowledge obeys its acceptance rules

| Type | Requirement |
| --- | --- |
| ID | REQ-TEMPLATE-ACCEPTANCE-RULES |
| Status | Accepted |
| Satisfies | [NEED-CONFORMANCE-FEEDBACK](product.mara.md#01M7BF6H6S3P42Y9TQ30T38WG6) |

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
| Term | No title or alias of it equals, after folding ([DES-FACT-BRIDGE](rules-facts.mara.md#01M7BF6HEHHJ7XZM8HDFXK115J)), a title or alias of another accepted Term. No alias is empty or equals its own title |
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
| Satisfies | [REQ-TEMPLATE-GATES-PROVE-READINESS](template-readiness.mara.md#01M7CT60MNF0XPXR9NB6X74F3B) |

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
