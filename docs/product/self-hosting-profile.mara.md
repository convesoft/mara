# Mara self-hosting flavour profile

This document describes the project-specific taxonomy used to develop Mara in
Mara. It does not define a built-in Mara methodology or a default project schema.

:::req m_01KY83G2DW8AT5YJ13ZY8GNXPM
:id: REQ-SELF-HOSTING-PROFILE
:title: Mara shall document its project-specific flavour profile
:status: approved
:level: system
:kind: quality
:priority: should
:derives_from: GOAL-SCHEMA-GENERIC

The Mara repository shall maintain schema-checked guidance defining the semantic
purpose, creation boundary, and exclusions of every flavour configured for its
self-hosting corpus. The guidance shall explicitly state that it is not a default
Mara process.
:::

:::decision m_01KY83DZDQMEFYNJ0A3SY3V30R
:id: ADR-0012
:title: Keep the self-hosting taxonomy project-specific
:status: accepted
:kind: process
:justifies: DES-MARA-SELF-HOSTING-TRACE

The twelve flavours in `.mara/schema.yaml` belong to the Mara project profile.
The engine shall not assume that another project has these flavours, lifecycle
values, relations, or traceability rules, and `mara init` shall not install them
as a default process.

This profile exists so Mara contributors and agents apply the current project
taxonomy consistently. The schema remains authoritative for machine validation;
this document explains modelling intent and boundaries.
:::

:::decision m_01KY83DZDRJNZASKD3K3D63J4N
:id: ADR-0013
:title: Use term for controlled project vocabulary
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `term` when a project concept needs one stable meaning across multiple
items or when ambiguity would harm reviews or agent context. Record synonyms on
the term and link semantic users through `uses_term`.

Do not create terms for ordinary words, temporary implementation names, or every
noun appearing in prose.
:::

:::decision m_01KY83DZDS6QDN189NKB4YS87X
:id: ADR-0014
:title: Use actor for durable interaction roles
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create an `actor` for a durable human role, agent, service, or external system
that participates in a story or scenario. Actors describe interaction roles and
system boundaries.

Do not use actors for current assignees, individual contributors, or delivery
ownership; those belong to the external work-management system.
:::

:::decision m_01KY83DZDTK09K1QAMD9HYBZN8
:id: ADR-0015
:title: Use goal for durable desired outcomes
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `goal` for a durable product, capability, or quality outcome explaining
why lower-level work exists. Goals may remain valid across many implementations
and releases.

Do not use goals for implementation tasks, milestones, or selected technical
solutions. Delivery priority and scheduling remain external operational state.
:::

:::decision m_01KY83DZDVTE63BS96GB3BJ8PD
:id: ADR-0016
:title: Use story for durable stakeholder outcomes
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `story` for a durable outcome desired by an actor. A story should remain
meaningful after the issues and pull requests that delivered it are closed, and
should normally derive from a goal and involve one or more actors.

Do not copy a Linear or GitHub issue into a story. Issues describe bounded work;
stories describe lasting product meaning.
:::

:::decision m_01KY83DZDWET0JXV6WRCS3P0F8
:id: ADR-0017
:title: Use scenario for concrete behavioural flows
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `scenario` when a concrete user, system, failure, or agent flow needs its
own identity and traceability. A scenario describes ordered interaction and
normally derives from a story or goal.

Do not use a scenario as a substitute for normative requirements. Requirements
state what must hold across applicable flows.
:::

:::decision m_01KY83DZDX7B7GKR7TWTN5K9VV
:id: ADR-0018
:title: Use req for atomic normative obligations
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `req` for an atomic, normative, and verifiable obligation on Mara. Its
body states what shall be true rather than how it is implemented. Requirements
should derive from durable intent and acquire verification links as they mature.

Do not use requirements for rationale, solution structure, temporary work, or
descriptive observations.
:::

:::decision m_01KY83DZDYD5XG8YRRDW1YNSRY
:id: ADR-0019
:title: Use design for solution structure and precise contracts
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `design` for solution structure or a precise implementation-facing
contract, including architecture, data models, algorithms, storage protocols,
CLI interfaces, and wire formats. A design should identify the requirements it
satisfies when those requirements exist.

Do not use design items to record why an option was selected; that rationale
belongs in a decision.
:::

:::decision m_01KY83DZDZVASXSA534M5EX3JJ
:id: ADR-0020
:title: Use decision for rationale and accepted trade-offs
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `decision` when Mara selects an option, policy, trade-off, or process
rule whose rationale should survive the implementation. Decisions justify
requirements or designs and may supersede earlier decisions.

Do not use a decision as the design definition itself or as a chronological work
log. The body records context, selected outcome, and relevant consequences.
:::

:::decision m_01KY83DZE0B13JH580P97S25G8
:id: ADR-0021
:title: Use risk for material uncertainty and adverse outcomes
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `risk` for a material uncertain condition that could harm product,
delivery, safety, compatibility, or maintainability. Link what it affects and the
requirements, designs, decisions, or tests that mitigate it.

Do not use risks for every known defect or active task. Concrete work belongs in
the delivery system unless the underlying uncertainty remains durable knowledge.
:::

:::decision m_01KY83DZE16VYP1P0J5F56HGQ2
:id: ADR-0022
:title: Use test for reusable verification or validation definitions
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create a `test` for a reusable definition of how a requirement or design is
verified, or how a goal, story, or scenario is validated. It specifies method,
level, inputs, and observable expectations independently of one execution.

Do not store a particular run result in a test item; concrete outcomes are
evidence.
:::

:::decision m_01KY83DZE2WQ4B689H24V97H7E
:id: ADR-0023
:title: Use evidence for immutable execution outcomes
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create `evidence` for one concrete passed, failed, or inconclusive execution of a
test, including capture time and commit or external provenance. Corrections add
new evidence and supersede the invalid record instead of rewriting its meaning.

Do not use evidence to define expected behaviour or test procedure.
:::

:::decision m_01KY83DZE3XE5ZW9MXG0R1E8ME
:id: ADR-0024
:title: Use artifact only for independently meaningful outputs
:status: accepted
:kind: process
:depends_on: ADR-0012
:justifies: DES-MARA-SELF-HOSTING-TRACE

Create an `artifact` only for an independently meaningful engineering output,
such as a crate, executable, command, API, schema, generated index, deployed
service, released report, or external standard. It may implement a requirement
or design and has an identity meaningful beyond its storage location.

Do not create an artifact merely because a `.mara.md` file contains items. Mara
documents, sections, source files, symbols, and spans are structural or derived
nodes unless the project deliberately manages one as a deliverable.
:::

:::design m_01KY83DZE4658A61YXV71WVRSM
:id: DES-MARA-SELF-HOSTING-TRACE
:title: Mara project traceability profile
:status: accepted
:kind: data_model
:depends_on: ADR-0012
:satisfies: REQ-SELF-HOSTING-PROFILE

The Mara project uses a flexible trace shape rather than a mandatory full chain:

```text
goal <- derives_from - story / scenario / req
story / scenario - involves -> actor
design - satisfies -> req
decision - justifies -> design / req
risk - affects -> project knowledge
req / design / decision / test - mitigates -> risk
test - verifies -> req / design
test - validates -> goal / story / scenario
evidence - evidences -> test
artifact / source span - implements -> req / design
any applicable item - uses_term -> term
```

Authors add only relations that carry useful meaning. The rules in
`.mara/schema.yaml`, not this illustrative shape, determine which links are
required for the Mara project at each lifecycle state.
:::

:::test m_01KY83G2DX97EERC0W8R1ZDTBX
:id: TEST-SELF-HOSTING-PROFILE
:title: Mara self-hosting flavour profile conformance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-SELF-HOSTING-PROFILE

A static corpus test shall compare the flavour names in `.mara/schema.yaml` with
the accepted flavour-boundary decisions in this document. It shall fail when a
configured flavour lacks guidance, when guidance names an absent flavour, or when
the project-specific and no-default-process boundary is missing.
:::
