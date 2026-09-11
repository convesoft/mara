# Migration to 0.2

Schema format 2 and mandatory flavour guidance are implemented starting with
`0.2.0-alpha.0`. Released 0.1 versions use schema format 1. Unified `search`,
`get`, and `related` are implemented.

## Schema guidance

Edit the existing `.mara/schema.yaml`: change `format_version` from 1 to 2 and
add guidance to every declared flavour under [[DES-FLAVOUR-AUTHORING-GUIDANCE]].
Preserve custom flavours, prefixes, fields, relations, and all item identities.
Do not reinitialize the project or replace its schema with a template.

Before (complete single-flavour example with a custom field and relation):

```yaml
format_version: 1
flavours:
  term:
    description: Controlled project vocabulary.
    id_prefix: TERM-
    body: required
    fields:
      alias:
        type: string
        repeatable: true
relations:
  clarifies:
    description: The source clarifies the meaning of the target term.
    source: [term]
    target: [term]
```

After:

```yaml
format_version: 2
flavours:
  term:
    description: Controlled project vocabulary.
    use_when:
      - Define a project-specific concept whose meaning needs clarification.
    avoid_when:
      - Repeat an ordinary word's dictionary meaning.
    distinguish_from: {}
    id_prefix: TERM-
    body: required
    fields:
      alias:
        type: string
        repeatable: true
relations:
  clarifies:
    description: The source clarifies the meaning of the target term.
    source: [term]
    target: [term]
```

Use meaningful guidance from existing project documentation. Empty `avoid_when`
and `distinguish_from` collections are valid when no exclusion or distinction
applies; `use_when` must be nonempty. Remove duplicate flavour definitions from
the taxonomy once transferred, keeping broader authoring policy and links.

With Mara 0.2, run `mara --format json schema validate` and
`mara --format json project validate` in the project root, or the equivalent
MCP `schema_validate` and `project_validate`. Require both to report
`valid: true`. Inspect the migrated declarations with `mara --format json
schema get` or MCP `schema_get`; use `schema get flavour term` or
`schema_get` with `{"kind":"flavour","name":"term"}` for one flavour.

Only the schema needs a format-version change. Keep `.mara/project.toml`, item
IDs, MIDs, metadata, and document bytes unchanged. Missing guidance or an invalid
guidance value is an error; format 1 is rejected with migration instructions and
is never upgraded automatically.

## Discovery commands and responses

Follow the compact [CLI/MCP migration mapping](discovery.mara.md#migration-from-01)
and its linked response contract. It covers renamed commands, removed options,
mixed-node response fields, and discarding old cursors.
