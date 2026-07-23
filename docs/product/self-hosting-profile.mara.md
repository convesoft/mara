# Mara self-hosting profile

This document defines why Mara uses its current project taxonomy and how those
flavours participate in project traceability. The schema itself carries the
authoritative per-flavour authoring guidance.

:::req m_01KY83G2DW8AT5YJ13ZY8GNXPM
:id: REQ-SELF-HOSTING-PROFILE
:title: Mara shall maintain a project-specific self-hosting profile
:status: approved
:level: system
:kind: quality
:priority: should
:derives_from: GOAL-SCHEMA-GENERIC

Every flavour configured in the Mara repository schema shall provide
schema-checked guidance defining its semantic purpose, creation criteria,
exclusions, and distinctions from confusable flavours. The profile shall state
that the configured taxonomy and traceability rules are specific to this project
and are not a built-in or default Mara process.
:::

:::decision m_01KY83DZDQMEFYNJ0A3SY3V30R
:id: ADR-0012
:title: Keep the self-hosting taxonomy project-specific
:status: accepted
:kind: process
:justifies: DES-MARA-SELF-HOSTING-TRACE

The twelve flavours in `.mara/schema.yaml` belong to the Mara project profile.
Their structured descriptions and selection guidance live beside their machine
constraints in the schema so contributors, agents, generated references, and
future interfaces consume one authoritative definition.

The engine shall not assume that another project has these flavours, lifecycle
values, relations, or traceability rules, and `mara init` shall not install them
as a default process. Each project must define its own taxonomy, guidance, and
traceability policy.
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
required for the Mara project at each lifecycle state. Flavour descriptions and
selection guidance in that schema determine which item kind represents a piece
of project knowledge.
:::

:::test m_01KY83G2DX97EERC0W8R1ZDTBX
:id: TEST-SELF-HOSTING-PROFILE
:title: Mara self-hosting flavour profile conformance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-SELF-HOSTING-PROFILE

A static corpus test shall inspect every flavour in `.mara/schema.yaml` and fail
when its label, description, creation criteria, exclusions, or cross-flavour
distinctions are missing or invalid. It shall also verify that this profile
contains one accepted project-taxonomy decision, that all distinction targets
resolve to configured flavours, and that the project-specific and
no-default-process boundary is preserved.
:::
