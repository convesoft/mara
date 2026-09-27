# Migration to 0.3

Use the 0.3 executable and matching [Mara skill](../skills/mara/SKILL.md).
Existing projects keep their own schema, documents, IDs and MIDs. Do not run
`project init` over a customized project. Mara has no automatic schema migration
command; [schema evolution](schema-evolution.mara.md) defines the reviewed source
edit and recovery procedure.

## Existing projects

1. Save a Git checkpoint or complete project copy. Inventory the project and
   schema format versions, custom declarations, authored relation assertions,
   included documents and configured rule files. Validate the original with
   its matching executable if available.
2. Change the existing schema to `format_version: 3`. A 0.2 schema already has
   the required flavour guidance; an older format 1 schema must add
   `description`, `use_when`, `avoid_when` and `distinguish_from` to every
   flavour. Preserve custom fields, relation meanings, documents and MIDs.
   Review item-body text that resembles `[[relation:target]]`, because 0.3
   interprets schema-declared typed links as relationship assertions.
3. Add inverse aliases, symmetric relations, external targets, structural
   policies or YAML rules only when the project intends those semantics.
   Existing projects gain none of the bundled engineering profile's status
   fields, relation vocabulary or accepted-knowledge rules automatically.
   Project format 1 remains valid; rules need project format 2 and explicit
   `[rules]` files, while code adapters need project format 3. Follow the
   [relation](relations.mara.md), [validation](validation.mara.md) and
   [code](code-traceability.mara.md) contracts for those opt-ins.
4. Review the complete diff and compare IDs/MIDs, unrelated fields, prose and
   links. Run `mara --format json schema validate` and
   `mara --format json project validate` with the 0.3 executable. Require
   `valid:true` and `evaluation_complete:true`; consume diagnostic pages through
   `has_more:false`. Inspect changed declarations with `schema get`, semantic
   edges and authored occurrences with `relation get`, and applicable rule
   results with `trace matrix`. Correct or restore an invalid candidate from
   the checkpoint. `project transaction rollback` applies only to an interrupted
   structured mutation, not manual schema edits.

Renaming or removing a relation alias requires reviewing every authored
occurrence. Replacing an inverse name with a canonical name on the same item
can reverse a valid edge. Changing direction, endpoints or meaning requires a
distinct intended relation and review of each affected assertion; validation
alone cannot establish the intended graph. The exact edit sequence is in
[[DES-SCHEMA-MIGRATION-WORKFLOW]].

## Clients and installation

Discovery, relationship, validation and trace results have independent public
format versions. Update clients to the [current interfaces](discovery.mara.md)
and [diagnostic codes](validation.mara.md); discard old cursors and structural
handles after changing the executable, schema or sources. Use `trace matrix`
or MCP `trace_matrix` for request-selected current-state coverage. Specification
export is retired. Code endpoints require a project-configured language indexer;
links describe associations and checks, not passing execution evidence.

The Agent Plugin package is no longer distributed. Install a supported native
package through the dispatcher, configure the stdio MCP command explicitly,
and install the matching standalone skill separately as shown in the
[README](../README.md). Supported hosts are listed in
[distribution](distribution.mara.md).
