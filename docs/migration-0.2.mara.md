# Migration to 0.2

Planned 0.2 migration, recorded during 0.1 release preparation. The current 0.1
loader does not support schema format 2; apply these steps when upgrading to 0.2.

## Schema guidance

Edit the existing `.mara/schema.yaml`: change `format_version` from 1 to 2 and
add guidance to every declared flavour under [[DES-FLAVOUR-AUTHORING-GUIDANCE]].
Preserve custom flavours, prefixes, fields, relations, and all item identities.
Do not reinitialize the project or replace its schema with a template.

Before (complete single-flavour example):

```yaml
format_version: 1
flavours:
  term:
    description: Controlled project vocabulary.
    id_prefix: TERM-
    body: required
    fields: {}
relations: {}
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
    fields: {}
relations: {}
```

Use meaningful guidance from existing project documentation. Empty `avoid_when`
and `distinguish_from` collections are valid when no exclusion or distinction
applies; `use_when` must be nonempty. Remove duplicate flavour definitions from
the taxonomy once transferred, keeping broader authoring policy and links.

With Mara 0.2, run `mara schema validate` and `mara project validate` in the
project root, or the equivalent MCP `schema_validate` and `project_validate`.
Require both to report valid. These are migration acceptance steps, not evidence
that the unimplemented 0.2 loader has been tested.

## Discovery commands and responses

Follow the compact [CLI/MCP migration mapping](discovery.mara.md#migration-from-01)
and its linked response contract. It covers renamed commands, removed options,
mixed-node response fields, and discarding old cursors.
