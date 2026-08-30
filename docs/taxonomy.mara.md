# Mara self-hosting taxonomy

This is Mara's project profile, not a built-in process imposed on other
projects. Use only flavours that represent durable knowledge; a trace may be
sparse and must not be completed with placeholder items.

| Flavour | ID prefix | Purpose |
|---|---|---|
| `term` | `TERM-` | Controlled project vocabulary |
| `actor` | `ACT-` | Durable participant role or external system |
| `goal` | `GOAL-` | Desired stakeholder or product outcome |
| `scenario` | `SCN-` | Concrete behavioural flow |
| `requirement` | `REQ-` | Independently verifiable obligation |
| `design` | `DES-` | Solution or interface contract |
| `decision` | `ADR-` | Consequential choice and rationale |
| `risk` | `RISK-` | Material uncertain adverse condition |
| `verification` | `VER-` | Repeatable verification or validation method |
| `evidence` | `EVD-` | Result of one verification execution |
| `artifact` | `ART-` | Concrete implementation or external output |

`story` is intentionally absent. Keep durable outcomes as goals, concrete flows
as scenarios, and temporary delivery work in the backlog or issue tracker.

:::mara term TERM-FLAVOUR-TERM
:title: Term

A term gives one stable meaning to project-specific vocabulary. Use it when
ambiguity would harm authoring or review. Do not itemize words with their
ordinary meaning or temporary implementation names.
:::

:::mara term TERM-FLAVOUR-ACTOR
:title: Actor

An actor is a durable human role, agent, service, or external system that
participates in behaviour. Use it when several items need to reference the
participant consistently. Do not use it for an individual assignee or temporary
delivery ownership.
:::

:::mara term TERM-FLAVOUR-GOAL
:title: Goal

A goal states a durable desired outcome and explains why lower-level work
exists. Use it when the outcome spans multiple scenarios or releases. Do not
use it for implementation tasks, milestones, or concrete interaction steps.
:::

:::mara term TERM-FLAVOUR-SCENARIO
:title: Scenario

A scenario describes one concrete user, system, failure, or agent flow with an
observable outcome. Use it to discover and validate behaviour. Do not use it
for obligations that must hold across multiple flows or for delivery tasks.
:::

:::mara term TERM-FLAVOUR-REQUIREMENT
:title: Requirement

A requirement states one independently verifiable obligation. Use it when
non-conformance can be determined from observable evidence. Do not combine
separable obligations or include rationale and solution choices as requirements.
:::

:::mara term TERM-FLAVOUR-DESIGN
:title: Design

A design defines how requirements are satisfied through architecture,
interfaces, formats, components, or algorithms. Use it when implementers need a
durable solution contract. Do not use it for product obligations or selection
rationale.
:::

:::mara term TERM-FLAVOUR-DECISION
:title: Decision

A decision records one consequential choice and why it was selected. Use it
when the rationale must survive implementation or when alternatives have
meaningful trade-offs. Do not record routine or easily reversible details.
:::

:::mara term TERM-FLAVOUR-RISK
:title: Risk

A risk records a material uncertain condition that could harm the product,
delivery, safety, or maintainability. Use it when exposure or mitigation should
influence durable work. Put known defects and ordinary follow-up work in the
backlog instead.
:::

:::mara term TERM-FLAVOUR-VERIFICATION
:title: Verification

A verification defines a repeatable test, inspection, analysis, or
demonstration and its observable expectations. Use it when the method itself
needs durable identity or traceability. Keep ordinary automated tests in code.
Do not record a particular execution result here.
:::

:::mara term TERM-FLAVOUR-EVIDENCE
:title: Evidence

Evidence records one concrete verification or validation result with enough
provenance to assess it. Use it only when that result needs durable auditability.
Do not duplicate routine CI results, logs, or the reusable verification method.
:::

:::mara term TERM-FLAVOUR-ARTIFACT
:title: Artifact

An artifact identifies a concrete implementation or external output whose
identity matters independently of its location. Use it for meaningful commands,
APIs, schemas, services, reports, or standards. Do not create one for every
source file, document, section, or symbol.
:::

## Relation vocabulary

These relations form the initial project traceability vocabulary. Use a bare
`[[ID]]` mention when navigation is useful but no typed meaning applies.

:::mara term TERM-RELATION-DERIVES-FROM
:title: derives_from

The source originates from or refines the target's intent. Use it for a direct
semantic basis, not chronology or a general association.
:::

:::mara term TERM-RELATION-DEPENDS-ON
:title: depends_on

The source cannot be satisfied, understood, or implemented independently of
the target. Do not use it merely because items are nearby or discussed together.
:::

:::mara term TERM-RELATION-SATISFIES
:title: satisfies

The source design provides a solution contract for the target requirement.
:::

:::mara term TERM-RELATION-JUSTIFIES
:title: justifies

The source decision preserves the reasoning for the target requirement or
design.
:::

:::mara term TERM-RELATION-SUPERSEDES
:title: supersedes

The source replaces an older target of the same flavour while preserving the
target as history. Do not use it for ordinary revisions of one item.
:::
