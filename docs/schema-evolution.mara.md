# Edit project vocabulary

Review schema declarations and authored assertions together when changing a
project-owned vocabulary. Keep a recoverable source checkpoint.

:::mara requirement REQ-SCHEMA-EVOLUTION
:mid: 01M3HK96WJEH0349W70DKPZ7QG
:title: Change project vocabulary without losing knowledge
:status: accepted
:kind: functional
:contributes_to: GOAL-UNIFIED-PROJECT-KNOWLEDGE

Provide a documented source-edit workflow for project-owned schemas and relation vocabulary. Explain affected declarations, authored references and validation consequences before edits. Preserve item MIDs and unrelated custom fields, prose and links. Initialization must not replace a customized project as a shortcut for schema editing.

Distinguish a label or alias change from changing relation direction, endpoint eligibility or meaning. Do not silently reinterpret existing edges. The workflow includes review, inspection, validation and recovery for manual changes; it does not promise an automated schema-editing command.
:::

:::mara design DES-SCHEMA-MIGRATION-WORKFLOW
:mid: 01M3HK9KV52ANP3HP6QAJNHCF1
:title: Edit schema declarations and authored references together
:status: accepted
:kind: behavior
:satisfies: REQ-SCHEMA-EVOLUTION

Schema and relation-vocabulary changes use manual edits on a Git checkpoint or recoverable project copy. The diff is the proposed change; the checkpoint/copy is the recovery source. project transaction rollback recovers only pending structured mutations under [[DES-MUTATION-RECOVERY]] and cannot undo manual edits. Inspect current schema/project formats, custom declarations, selected source files, IDs/MIDs, authored relation spellings and configured rule paths before changing them. Current supported format and guidance constraints follow [[DES-FLAVOUR-AUTHORING-GUIDANCE]] and [[DES-SCHEMA-RULE-DEFINITIONS]].

Adding an inverse alias leaves existing canonical assertions and their direction intact. Renaming an alias requires changing its declaration and every authored metadata/typed-inline occurrence using that alias; preserve canonical source and target. Removing an alias requires inspecting each canonical edge and its occurrences, ensuring a canonical assertion exists on its canonical source, removing inverse metadata and demoting inverse inline tokens to bare mentions where prose navigation should remain. Then remove the inverse declaration. Replacing an inverse name with the canonical name on the same item can reverse a valid directed edge when both endpoints have eligible flavours; validation alone does not establish intended direction.

Renaming a canonical relation without changing meaning requires updating its declaration, authored metadata/typed-inline spellings, YAML path/inversePath references and clients that name it. Keep endpoint eligibility, direction, external mode and policy with that declaration. A change to direction, endpoints or meaning requires a distinct intended relation and individual review of affected assertions/rules; keep the existing declaration until the intended new edges are valid. Do not use a blind text replacement over prose and literal examples.

Before accepting a change, review the complete diff and compare IDs/MIDs, custom fields, unrelated prose, links, source files and project settings. Never regenerate existing MIDs. Run schema validate and project validate with the matching executable, require valid and evaluation_complete, and consume every diagnostic page with unchanged inputs. Inspect changed schema declarations and canonical relation endpoints/occurrences through schema get and relation get. If policy changed, inspect applicable validation and a selected matrix. Correct an invalid candidate or restore its source checkpoint; an invalid intermediate state is not a completed edit.
:::

:::mara decision ADR-MANUAL-SCHEMA-MIGRATION
:mid: 01M3HKA0HPV4Z75NGQWXSCA7PZ
:title: Use reviewed source edits for schema evolution
:status: accepted
:justifies: DES-SCHEMA-MIGRATION-WORKFLOW

Project files are already the authoring authority, and Git or a complete copy provides a reviewable proposal and recovery point. Manual source edits followed by explicit inspection and validation support deliberate vocabulary changes without guessing whether changed directions, endpoint sets or meanings are equivalent. This keeps schema evolution separate from the structured item-mutation journal.
:::

:::mara verification VER-SCHEMA-EVOLUTION
:mid: 01M3HKAD0SBWS7ST0CHGC3APYF
:title: Verify customized schema edits and canonical edge direction
:status: accepted
:method: test
:verifies: REQ-SCHEMA-EVOLUTION
:verifies: DES-SCHEMA-MIGRATION-WORKFLOW

Run cargo test --locked --test schema_evolution --test schema_validation with the candidate CLI and real stdio MCP server in disposable projects with isolated Git/configuration state. Add an inverse alias to a customized current-format schema and author both metadata and inline inverse assertions. Require one canonical edge with all occurrences. Rename the declaration alone, require invalid validation without source rewrites, then update authored spellings and require valid equivalent CLI/MCP results. Preserve custom enum fields, item MIDs, unrelated document bytes and project settings.

For an inverse whose source and target share a flavour, demonstrate that replacing the inverse name on the same author can validate while reversing the edge. Apply the correct sequence: author the canonical source, remove inverse metadata, demote inverse inline tokens to mentions, and remove the alias declaration. Require the intended canonical edge, absence of the reversed edge and exact intended source bytes through real transports. Definition tests own unsupported-format/guidance validation; [[VER-NPM-DISTRIBUTION]] exercises packaged custom-schema repair.

Review the guide against these actual operations and current command names. Run formatting, Clippy, the full relevant regression suite and complete candidate schema/project validation. Record the actual tested revision and scope, and consume every selected traceability page. These checks demonstrate manual source editing plus Mara inspection/validation, not an automated transformation or recovery command.
:::
