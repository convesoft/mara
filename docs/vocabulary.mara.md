# Engineering vocabulary

## Design

---

<a id="01M7BF6HH8CA2B9TQJ9DT32WDG"></a>

### Types and relations of the engineering template

| Type | Design |
| --- | --- |
| ID | DES-ENGINEERING-VOCABULARY |
| Status | Accepted |
| Satisfies | [REQ-PROJECT-INIT](project.mara.md#01M7BF6H8Z02SNKXD2XPGVD87M) |

Every type has `Status`: `Draft`, `Accepted` or `Retired`.

| Type | ID prefix | Use for | Extra fields |
| --- | --- | --- | --- |
| Goal | `GOAL-` | A desired stakeholder or engineering outcome and why it matters | |
| Need | `NEED-` | A stakeholder capability, condition or result, without prescribing realization | |
| Scenario | `SCN-` | A concrete operational situation or flow and its outcomes, in prose | |
| Requirement | `REQ-` | One precise, necessary, verifiable obligation on the solution | |
| Design | `DES-` | A durable interface, format, boundary or realization | |
| Decision | `ADR-` | A consequential choice, its alternatives, context and rationale | |
| Risk | `RISK-` | An uncertain cause with an adverse consequence | `Likelihood`: Rare, Unlikely, Possible, Likely, Almost-certain. `Consequence`: Minimal, Minor, Moderate, Major, Severe |
| Term | `TERM-` | A canonical definition whose ambiguity affects engineering work | `Aliases`: repeatable text |
| Evidence | `EVD-` | A durable empirical or analytical conclusion with provenance, not a routine CI run | |
| Verification | `VER-` | How requirement conformity is established | `Methods`: repeatable; Test, Analysis, Inspection, Demonstration |
| Validation | `VAL-` | How need satisfaction or goal achievement is established | `Methods`: repeatable; Test, Analysis, Inspection, Demonstration |

| Key on the source item | Shown on the target as | Source → target types | Constraint | Means |
| --- | --- | --- | --- | --- |
| Contributes to | Contributed to by | Need, Design → Goal | | The source helps achieve the target |
| Satisfies | Satisfied by | Requirement → Need; Design → Requirement | | The source is intended to fulfil the target |
| Decomposes | Decomposed by | Need, Requirement, Design → the same type | Child → parent | The source is a part of the target |
| Derives from | Gives rise to | Requirement → Requirement | | The source follows from the target |
| Supersedes | Superseded by | Goal, Need, Scenario, Requirement, Design, Decision, Term, Verification, Validation → the same type | No cycles | The source replaces the target |
| Justifies | Justified by | Decision → Requirement, Design | | The source gives the reason for the target |
| Accepts | Accepted by | Decision → Risk | | The source accepts the target risk as it stands |
| Threatens | Threatened by | Risk → Goal, Need, Requirement | | The source endangers the target |
| Mitigates | Mitigated by | Requirement, Design → Risk | | The source reduces the target risk |
| Verifies | Verified by | Verification → Requirement | | The source establishes conformity to the target |
| Validates | Validated by | Validation → Need, Goal | | The source establishes that the target is satisfied or achieved |
| Exercises | Exercised by | Validation → Scenario | | The source runs through the target |
| Supports | Supported by | Evidence → Goal, Need, Requirement, Design, Decision, Risk | | The source is evidence for the target |

`Implements` (code → Requirement, Design, Verification; shown on the target as `Implemented by`) comes only from code markers ([DES-CODE-MARKERS](code.mara.md#01M7BF6GQ1G370S0G7HPN4HGY1)), never from a metadata table. Its schema description is "The code realizes the target."; every other description is the `Use for` or `Means` text above, ending with a full stop.

The template's rules, queries, gates and views are on [knowledge rules](template.mara.md) and [readiness](template-readiness.mara.md).
