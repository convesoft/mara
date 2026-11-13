# Mara: Markdown-Native Item Format — Design Handoff

## Goal

Replace Mara's custom `:::mara` directive-based item syntax with ordinary Markdown constructs while preserving structured knowledge, schema validation, stable identity, relationships, and graph indexing.

**Core principle:** A single Markdown file must serve as both the editable source and the published documentation (including GitHub rendering). No export, compilation, or separate publication format.

The motivation is improved readability, syntax highlighting, native Markdown rendering, and native link navigation without sacrificing Mara's structured semantics.

Repository: https://github.com/convesoft/mara

Mara is being reimplemented from scratch; this format targets the rewrite, not the existing parser.

## Format

```markdown
# Authentication

How users sign in.

---

<a id="01M1PXP2KGVW5ZF2JGP9K4XE9B"></a>

## Phishing-resistant authentication

| Type      | Requirement |
| --------- | ----------- |
| ID        | REQ-PHISHING-RESISTANT-AUTH |
| Status    | Accepted |
| Satisfies | [NEED-SECURE-SIGN-IN](../needs.mara.md#01M1PXQ0R8T4D2VW6YH3KJ5N7A) |

Users must authenticate using phishing-resistant MFA.

### Acceptance criteria

- Passkeys must be supported.
- Password-only authentication must be rejected.

---

<a id="01M1PXP3KGVW5ZF2JGP9K4XE9C"></a>

## Use WebAuthn

| Type      | Decision |
| --------- | -------- |
| ID        | ADR-USE-WEBAUTHN |
| Status    | Accepted |
| Justifies | [REQ-PHISHING-RESISTANT-AUTH](#01M1PXP2KGVW5ZF2JGP9K4XE9B) |

WebAuthn provides phishing-resistant authentication.

---

Closing narrative that belongs to no item.
```

## Decisions

### 1. Documents and regions

- Mara documents are `*.mara.md` files selected by configured include patterns; other `.md` files are never scanned.
- Every thematic break (`---`, `***`, `___`) outside code is a region boundary. BOF and EOF are implicit boundaries.
- A region whose first block is a MID anchor is an item. Any other region is ordinary Markdown.
- Items end only at a thematic break or EOF. Headings never end an item, so items may contain nested headings and prose may follow an item without a heading.
- A MID anchor that is not the first block of its region is an error (missing separator).
- Items cannot contain thematic breaks and cannot nest.
- Setext headings are errors: `text` directly above `---` would turn the separator into a heading and leak the next region into the item. Always surround `---` with blank lines.

### 2. Item identity

- MID is a canonical uppercase 26-character ULID, stored only in an HTML anchor: `<a id="<MID>"></a>`.
- The anchor is its own paragraph, followed by a blank line and the item heading. An anchor merged into another paragraph is an error.
- The anchor provides both immutable Mara identity and a stable GitHub fragment target.
- GitHub renders the anchor as `id="user-content-<lowercase MID>"` but keeps link hrefs as written, and its page script matches fragments case-insensitively on page load and on in-page clicks (verified 2026-11-13 for same-file, cross-file, and table-cell links). Anchors and links therefore keep the canonical uppercase MID.
- MID never changes when the title, human-readable ID, or file location changes.

### 3. Item structure

An item region contains, in order:

1. MID anchor.
2. ATX heading: the title, and nothing else. IDs are never extracted from heading text.
3. Metadata table.
4. Body: all remaining blocks, any Markdown (paragraphs, headings, lists, tables, code). Every flavour requires a non-empty body.

Every valid item requires MID, title, `Type`, `ID`, and `Status`. `Type` names a schema-defined flavour; it is not a hardcoded enumeration.

### 4. Metadata table

- The metadata table is the first block after the heading: a two-column GFM table where every row, including the header row, is `Key | Value`.
- The header row holds the first property; the writer puts `Type` there. GFM requires a header row, so this avoids a filler row.
- Property order is otherwise free.
- Multi-valued properties repeat the key row, one value per row. No comma-separated values.
- Property semantics, cardinality, and allowed values come from the schema. No nested metadata objects.
- `|` inside a value is escaped as `\|`. Column alignment padding is optional.
- The table ends at the first blank line, so the body may start with any block. A non-blank line directly after the table becomes a malformed row and is an error.

### 5. Labels

- Keys and enum values are written as labels derived from schema names: replace `_` with a space and capitalize the first letter (`derives_from` → `Derives from`, `accepted` → `Accepted`, `almost-certain` → `Almost-certain`). `Type` values are capitalized flavour names (`Requirement`).
- Reading matches labels case-insensitively; writing uses the derived label.

### 6. Human-readable IDs

- Every item has exactly one `ID` row. IDs are mandatory for every flavour; there is no schema switch.
- An ID starts with its flavour's prefix (see Vocabulary). Prefer a readable slug: `REQ-PHISHING-RESISTANT-AUTH`.
- The ID is the relation link text, the CLI/MCP handle, and the grep target. MID remains the canonical identity; the ID is a renameable alias.
- The CLI requires an ID or proposes one from the flavour prefix and a title slug.

### 7. Metadata keys and values

- Keys not defined by the schema for the item's flavour are errors, including keys valid only for other flavours. Values outside a field's allowed set are errors.
- Extra fields are added by declaring them in the schema; there is no "allow unknown keys" setting.
- An item with invalid rows still loads; invalid rows are reported, never silently dropped.

### 8. Relations and references

- Relations are metadata rows on the source item, keyed by the forward relation name. Inverse names (`Satisfied by`) are for reading and navigation only.
- Relation values are Markdown links: `[<ID>](<relative-path>#<MID>)`, or `[<ID>](#<MID>)` within the same file. They navigate natively on GitHub.
- The fragment MID is the identity. The path must be the file currently containing that MID, relative to the linking file. The link text is the target's current ID.
- Stale link paths, stale link text, and bare-ID relation values are diagnostics; the fix operation rewrites them to canonical links.
- Moving an item rewrites inbound link paths and the moved item's outbound relative paths. Renaming an ID rewrites inbound link text.
- Any body or narrative link whose fragment is a known MID is a mention. There is no `[[ID]]` or `[[relation:target]]` syntax; typed relations exist only in the metadata table.

### 9. Recognition and validation

- Anchor + heading + `Type` is an item.
- An anchor region missing its heading or `Type`, or a region starting with a heading followed by a metadata table with a `Type` row but no anchor, is an incomplete item: reported, retained in diagnostics, and repairable by the fix operation.
- Ordinary Markdown without these markers is never an item.
- Parsing and validation are read-only; only the explicit fix operation rewrites source.

### 10. Migration

- The existing `:::mara` corpus is converted once by a one-shot converter; the rewrite does not parse both formats.

## Vocabulary

Source: `templates/engineering-schema.yaml` on branch `plan-execution-architecture` (new engineering template).

### Types

Every type requires `Status`: `Draft`, `Accepted`, or `Retired`.

| Type | ID prefix | Use for | Extra fields |
| --- | --- | --- | --- |
| Goal | `GOAL-` | A desired stakeholder or engineering outcome and why it matters. | — |
| Need | `NEED-` | A stakeholder capability, condition, or result, without prescribing realization. | — |
| Scenario | `SCN-` | A concrete operational situation or flow and its outcomes, in prose. | — |
| Requirement | `REQ-` | One precise, necessary, verifiable obligation on the solution. | — |
| Design | `DES-` | A durable interface, format, boundary, or realization. | — |
| Decision | `ADR-` | A consequential choice, its alternatives, context, and rationale. | — |
| Risk | `RISK-` | An uncertain cause with an adverse consequence. | `Likelihood`: Rare, Unlikely, Possible, Likely, Almost-certain. `Consequence`: Minimal, Minor, Moderate, Major, Severe. |
| Term | `TERM-` | A canonical definition whose ambiguity affects engineering work. | `Aliases`: repeatable text. |
| Evidence | `EVD-` | A durable empirical or analytical conclusion with provenance (not routine CI runs). | — |
| Verification | `VER-` | How requirement conformity is established. | `Methods`: repeatable; Test, Analysis, Inspection, Demonstration. |
| Validation | `VAL-` | How need satisfaction or goal achievement is established. | `Methods`: repeatable; Test, Analysis, Inspection, Demonstration. |

### Relations

| Key on source item | Source types | Target types | Constraint |
| --- | --- | --- | --- |
| Contributes to | Need, Design | Goal | |
| Satisfies | Requirement, Design | Need, Requirement | |
| Decomposes | Need, Requirement, Design | Need, Requirement, Design | Same type; child → parent |
| Derives from | Requirement | Requirement | |
| Supersedes | Goal, Need, Scenario, Requirement, Design, Decision, Term, Verification, Validation | Same set | Same type; no cycles |
| Justifies | Decision | Requirement, Design | |
| Accepts | Decision | Risk | |
| Threatens | Risk | Goal, Need, Requirement | |
| Mitigates | Requirement, Design | Risk | |
| Verifies | Verification | Requirement | |
| Validates | Validation | Need, Goal | |
| Exercises | Validation | Scenario | |
| Supports | Evidence | Goal, Need, Requirement, Design, Decision, Risk | |

`Implements` (code → Requirement, Design, Verification) comes only from code references, never from document tables.

## Authoring workflow

Mara is primarily used through CLI and MCP authoring operations.

- Centralize anchor, table, and link generation in one writer.
- Generate MID and anchor together when creating items.
- Preserve anchors during updates and moves; maintain relation links on rename and move.
- Provide one explicit fix operation for incomplete items, bare IDs, stale links, and malformed formatting.
- Preserve user-authored Markdown formatting wherever possible.

## Writing documents before the tool exists

Until the rewrite has a writer and fix operation, write items by hand:

- Generate each MID with:

  ```sh
  python3 -c "import os,time;n=(int(time.time()*1000)<<80)|int.from_bytes(os.urandom(10),'big');a='0123456789ABCDEFGHJKMNPQRSTVWXYZ';print(''.join(a[(n>>(5*i))&31] for i in range(25,-1,-1)))"
  ```

- Never copy or reuse a MID. Keep link paths and link text current by hand when moving items or renaming IDs.
