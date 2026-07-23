# Mara engineering knowledge

This directory is the canonical engineering-knowledge corpus for Mara. Files
provide readable narrative context; Mara items provide stable identity,
schema-checked semantics, and typed traceability. Traditional deliverables such
as a product requirements document, software requirements specification,
architecture description, verification specification, or traceability matrix
are views over this corpus rather than independent sources of truth.

## Product context

- [Product charter](product/charter.mara.md)
- [Human and agent workflows](product/workflows.mara.md)
- [Mara self-hosting flavour profile](product/self-hosting-profile.mara.md)

## Capability specifications

- [Project and content system](capabilities/project-system.mara.md)
- [Schema system](capabilities/schema-system.mara.md)
- [Authoring language](capabilities/authoring-language.mara.md)
- [Identity and references](capabilities/identity-and-references.mara.md)
- [Validation](capabilities/validation.mara.md)
- [Exploration and indexing](capabilities/exploration-and-indexing.mara.md)
- [Git editing](capabilities/git-editing.mara.md)
- [Agent context and future integrations](capabilities/agent-context.mara.md)

## Cross-cutting specifications

- [System architecture](architecture/system.mara.md)
- [Verification strategy](verification/strategy.mara.md)

## Normative references

- [Project format v1](reference/project-format.mara.md)
- [Schema format v1](reference/schema-format.mara.md)
- [Transaction journal v1](reference/transaction-journal.mara.md)
- [Wire contracts v1](reference/wire-contracts.mara.md)

The v0.1 implementation scope is the local deterministic CLI described by
[[GOAL-BOOTSTRAP]] and its approved requirements. Proposed requirements in the
agent-context specification preserve explicit post-v0.1 boundaries without
expanding the bootstrap release.
