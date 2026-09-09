# Mara self-hosting taxonomy

This is Mara's project profile, not a built-in process imposed on other
projects. Use only flavours that represent durable knowledge; a trace may be
sparse and must not be completed with placeholder items.

The [project schema](../.mara/schema.yaml) owns flavour descriptions, selection
guidance, ID prefixes, and constraints under [[DES-FLAVOUR-AUTHORING-GUIDANCE]].
Use `mara schema list flavour` to discover names, then `mara schema get flavour
<name>` for the authoritative declaration (MCP `schema_list` and `schema_get`).

`story` is intentionally absent. Keep durable outcomes as goals, concrete flows
as scenarios, and temporary delivery work in the backlog or issue tracker.

## Relation vocabulary

These relations form the initial project traceability vocabulary. Use a bare
`[[ID]]` mention when navigation is useful but no typed meaning applies.

:::mara term TERM-RELATION-DERIVES-FROM
:mid: 01M1PXP2KGZXAJ1595RM3RN7AC
:title: derives_from

The source originates from or refines the target's intent. Use it for a direct
semantic basis, not chronology or a general association.
:::

:::mara term TERM-RELATION-DEPENDS-ON
:mid: 01M1PXP2KGH48HVPK1H8WRXT22
:title: depends_on

The source cannot be satisfied, understood, or implemented independently of
the target. Do not use it merely because items are nearby or discussed together.
:::

:::mara term TERM-RELATION-SATISFIES
:mid: 01M1PXP2KGVRSX3829GYHRKV90
:title: satisfies

The source design provides a solution contract for the target requirement.
:::

:::mara term TERM-RELATION-JUSTIFIES
:mid: 01M1PXP2KGNBEQN8SJ6ZPKNHSS
:title: justifies

The source decision preserves the reasoning for the target requirement or
design.
:::

:::mara term TERM-RELATION-SUPERSEDES
:mid: 01M1PXP2KG05Y3Z9HX3MY27VH8
:title: supersedes

The source replaces an older target of the same flavour while preserving the
target as history. Do not use it for ordinary revisions of one item.
:::
