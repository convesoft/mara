# Migration to 0.3

Use an executable and packaged skill from the same 0.3 release or source
revision. Published 0.2 executables do not accept schema format 3. This guide
applies to one existing project; keep its own schema, configuration and Mara
documents rather than initializing a replacement project.

:::mara design DES-SCHEMA-MIGRATION-WORKFLOW
:mid: 01M3C1KKDS0THFNRXB7KSDF489
:title: Migrate 0.3 formats and relation vocabulary through reviewed source edits
:satisfies: REQ-SCHEMA-EVOLUTION

The supported 0.3 migration is manual. Mara provides validation and inspection,
but no schema migration preview/apply command. A Git diff or comparison of a
recoverable copy is the proposed change; the checkpoint or original copy is
the recovery source. Do not use `project transaction rollback` for manual edits:
it recovers a pending structured mutation journal only.

Supported changes within this workflow are:

| Change | Required source edit | Semantic boundary |
|---|---|---|
| Schema 1 or 2 to 3 | Change the existing schema version; add flavour guidance if starting at 1. | Existing relations remain directed, internal and without aliases unless explicitly changed. |
| Add an inverse alias | Add `inverse` to one eligible canonical relation. | Existing canonical assertions retain their direction and meaning. |
| Rename an inverse alias | Change the declaration and every authored metadata or typed-inline occurrence using the old alias. | The authoring endpoint and canonical edge stay the same. Do not treat a separately declared reverse relation as an alias. |
| Remove an inverse alias | For each affected edge, ensure a canonical assertion exists on its canonical source. Remove every inverse metadata occurrence and demote inverse inline tokens to bare mentions while preserving prose; then remove `inverse` from the declaration. | Replacing the alias name with the canonical name on the same item reverses a directed edge when both endpoint flavours are eligible; validation alone can miss this. |
| Rename a canonical relation without changing its meaning | Rename its declaration key and every authored metadata or typed-inline occurrence using that name; update YAML rule `path` and `inversePath`, and clients that name the relation. | Keep endpoint sets, direction, external mode and policy with the same declaration. A new name alone does not change edge meaning. |
| Opt in to 0.3 policy | Add reviewed schema `cardinality`/`acyclic` constraints or YAML rule files and project rules configuration. | Existing projects gain no policy, statuses, owners, evidence or links automatically. |

For a schema starting at format 1, use the [0.2 guidance example](migration-0.2.mara.md#schema-guidance)
to supply the four required flavour guidance keys while setting the final
schema version to 3.

Changes to direction, endpoint eligibility or meaning are **not** relation
renames or alias edits. Define the intended relation under a distinct canonical
name and review each affected assertion and rule before replacing the old
declaration. A narrowed endpoint set can invalidate existing edges; reversing
a directed relation can
change every edge's source and target. Keep the old declaration until the new
assertions are valid and their intended meaning is confirmed. No automatic
conversion of these changes is supported. Field or flavour renames and
arbitrary schema restructuring have no 0.3 migration operation.

## Manual application and recovery

1. Save a Git checkpoint or a complete project copy. Inventory schema and
   project versions, custom declarations, included source files, item ID/MID
   pairs, authored relation spellings, rule paths and configured rule files.
   Validate the original with its matching executable when available.
2. Prepare edits on the checkpoint branch or copy. Before enabling schema 3,
   escape item-body examples that would become meaningful
   `[[relation:target]]` assertions. Check inverse names against canonical
   names, aliases, structural keys and eligible custom fields. Inspect every
   occurrence of a renamed relation in metadata, item bodies and YAML rules.
   For alias removal, inspect the canonical edge and its occurrences first;
   migrate assertions before deleting the alias declaration. Recheck canonical
   source and target after the edit, not just project validity. Code blocks and
   narrative text may contain literal examples and require
   human review rather than a blind global replacement.
3. Review the complete diff before accepting it. Only the intended schema,
   project configuration, rules and authored assertions may differ. Compare
   every ID/MID pair and check that unrelated fields, prose, links, source
   files and custom declarations are unchanged. Never regenerate MIDs.
4. Run `mara --format json schema validate` and `mara --format json project
   validate` with the matching 0.3 executable, or MCP `schema_validate` and
   `project_validate`. Require `valid:true` and `evaluation_complete:true`
   from both; follow diagnostic cursors with unchanged inputs until
   `has_more:false`. Inspect changed declarations with `schema get`/`schema_get`
   and representative edges with `relation get`/`relation_get`, including
   occurrence counts and endpoint-facing labels. If rules changed, compare
   applicable validation results and a selected trace matrix.
5. If a candidate is invalid, correct it against the checkpoint or restore the
   original copy and retry. An invalid candidate is not a completed migration.
   Only keep a reviewed diff after full validation passes.

## Before and after

An existing customized schema can gain an inverse label without moving its
verification item or rewriting the canonical assertion. The custom `status`
field, MID, prose and link remain byte-for-byte unchanged:

```yaml
# Before, schema format 2
format_version: 2
flavours:
  requirement:
    description: A project obligation.
    use_when: [State an obligation.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: required
    fields: {}
  verification:
    description: A repeatable check.
    use_when: [Define a check.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: VER-
    body: required
    fields:
      status: {type: enum, values: [draft, approved]}
relations:
  verifies:
    description: The verification checks the requirement.
    source: [verification]
    target: [requirement]
```

```yaml
# After, schema format 3
format_version: 3
flavours:
  requirement:
    description: A project obligation.
    use_when: [State an obligation.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: REQ-
    body: required
    fields: {}
  verification:
    description: A repeatable check.
    use_when: [Define a check.]
    avoid_when: []
    distinguish_from: {}
    id_prefix: VER-
    body: required
    fields:
      status: {type: enum, values: [draft, approved]}
relations:
  verifies:
    description: The verification checks the requirement.
    source: [verification]
    target: [requirement]
    inverse: verified_by
```

The existing `VER-A` assertion `:verifies: REQ-A` still establishes
`VER-A → REQ-A`. `REQ-A` can now author `:verified_by: VER-A`; that is another
occurrence of the same edge. Further changes have different effects:

| Change | Before | After | Review consequence |
|---|---|---|---|
| Inverse label | `inverse: verified_by`; `REQ-A` has `:verified_by: VER-A` and `[[verified_by:VER-A]]`. | `inverse: checked_by`; both occurrences use `checked_by`. | Same canonical `verifies` edge and direction. |
| Remove inverse label | `REQ-A` has `:verified_by: VER-A` and `[[verified_by:VER-A]]` for `VER-A → REQ-A`. | Add `:verifies: REQ-A` on `VER-A` if absent; remove the inverse metadata on `REQ-A`, change its inline token to `[[VER-A]]`, then remove `inverse`. | Retains `VER-A → REQ-A` and prose navigation; writing `:verifies: VER-A` on `REQ-A` would reverse the edge when eligible. |
| Canonical name | `verifies`; `VER-A` has `:verifies: REQ-A`; rule uses `{inversePath: verifies}`. | `checks`; metadata and rule path use `checks`. | Keep endpoint sets and policy; update clients naming the relation. |
| Canonical direction | `verifies` has `source: [verification]`, `target: [requirement]`; `VER-A` has `:verifies: REQ-A`. | New `checked_by_verification` has `source: [requirement]`, `target: [verification]`; `REQ-A` has `:checked_by_verification: VER-A`. | Review each assertion and rule because the canonical edge direction changes; retain the old declaration until replacement is valid. |
| Endpoint eligibility | `verifies` targets `[requirement, design]`; `VER-A` has `:verifies: DES-A`. | Target narrows to `[requirement]`; the `DES-A` assertion is removed or deliberately reauthored under a suitable distinct relation. | The old assertion becomes invalid; no alias can make `DES-A` eligible. |
| Meaning | `verifies` links a verification item to a requirement. | A separate `tracked_by` relation points from `REQ-A` to `external:https://example.com/ticket/7`. | A ticket address has no MID or verification status; renaming `verifies` would falsely reinterpret the old edge. |

The silent reversal is visible when both endpoints are requirements: with
`follows` from requirement to requirement and inverse `followed_by`, an
assertion `:followed_by: REQ-A` on `REQ-B` means `REQ-A follows REQ-B`.
Changing that line in place to `:follows: REQ-A` passes validation but means
`REQ-B follows REQ-A`. To remove the alias, author `:follows: REQ-B` on
`REQ-A`, remove the old metadata line on `REQ-B`, and demote an old
`[[followed_by:REQ-A]]` token there to `[[REQ-A]]` before deleting `inverse`.

## Persisted and public compatibility

- Schema format 3 activates typed inline relationships, inverse/symmetric and
  external relation declarations, and optional structural policies. It rejects
  schema 1/2 until migrated. Documents retain their delimiters and MIDs and
  have no separate format marker.
- Project format 1 remains valid without conditional rule sources. Enabling
  YAML rules requires project format 2 with `[rules]`, binding
  `format_version = 1`, and explicit project-relative files. Invalid rule
  definitions prevent successful adoption. No rule context or Turtle files
  are migrated; those development formats were never shipped.
- Discovery JSON format 2, relationship JSON format 1, validation JSON format
  1 and trace JSON format 1 are separate client contracts. Clients must update
  their parsers and discard old cursors and structural handles. MIDs remain
  stable. Validation clients use `code`, `severity`, `valid` and
  `evaluation_complete`; matrices are read-only projections. MCP transport
  negotiation is unchanged, but tool schemas reflect these domain results.

The detailed relationship boundary is [[DES-RELATION-COMPATIBILITY]]; rule,
validation and matrix boundaries are [[DES-TRACE-CONTRACT-COMPATIBILITY]].
:::

:::mara decision ADR-MANUAL-SCHEMA-MIGRATION
:mid: 01M3C1KKDSAFZRK7ERF4T0ZQBZ
:title: Use reviewed manual migration for 0.3
:justifies: DES-SCHEMA-MIGRATION-WORKFLOW

Use source edits on a Git checkpoint or recoverable copy, followed by diff
review and full schema/project validation. This covers the demonstrated format
upgrade and relation-name changes without introducing a migration command or
guessing whether a changed direction, endpoint set or meaning is equivalent.
The source files are already the authoring authority and Git supplies a
reviewable proposal and recovery point. Revisit automation only when a
repeated, well-defined transformation justifies a preview/apply contract.
:::
