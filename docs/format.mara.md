# Canonical source and item identity

Documents remain readable Markdown; structured operations work on those files.
Parsing preserves authored content and locations. Validation adds schema and
identity checks without silently rewriting source.

:::mara requirement REQ-CANONICAL-SOURCE
:mid: 01M1PXP2KGFTACAY742WPSWRK7
:title: Git-tracked Mara documents remain canonical
:status: accepted
:derives_from: SCN-START-STRUCTURED-PROJECT
:kind: constraint

Mara must read and write `*.mara.md` without making a database, generated
index, MCP response, CLI response, or other projection an authoring authority.
Direct reading, editing, and `ripgrep` remain supported workflows.
:::

:::mara requirement REQ-DURABLE-ITEM-IDENTITY
:mid: 01M1PXP2KG5R46BBV7ZGQE6XGB
:title: Give every item a durable machine identity
:status: accepted
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY
:kind: functional

Every item has one repository-wide immutable MID in addition to its
human-readable ID. A MID is a raw canonical uppercase 26-character ULID with no
prefix. Human IDs remain readable handles and authored relation targets, while
identity-dependent resolution can use either an exact MID or an exact human ID
deterministically. Duplicate MIDs and duplicate human IDs are invalid even when
the other identity differs.
:::

:::mara requirement REQ-MID-BACKFILL
:mid: 01M1PXP2KG07XC4AX4G6X7BSNC
:title: Backfill missing item identities deliberately
:status: accepted
:derives_from: SCN-AUTHOR-ITEM-FLEXIBLY
:kind: functional

`project mid backfill` and MCP `project_mid_backfill` add generated MIDs to
every legacy item that lacks one. Backfill preserves existing source content and
existing MIDs, writes each new `:mid:` immediately after its item opener, and
makes no source changes when its validation preflight fails. Normal reads never
backfill automatically.
:::

:::mara design DES-DURABLE-ITEM-IDENTITIES
:mid: 01M1PXP2KGC400ND3WXDFZJE88
:title: Store machine identities as structural metadata
:status: accepted
:satisfies: REQ-DURABLE-ITEM-IDENTITY
:satisfies: REQ-MID-BACKFILL
:satisfies: REQ-SURFACE-PARITY
:kind: data

The in-memory item projection retains both the authored human ID and the
generated MID. Query and mutation operations resolve item handles by exact MID
when the handle is a canonical MID, otherwise by exact human ID. Compact and
resolved item structures include both identities when a MID is present.

Resolved relation traversal resolves target handles to item identities
internally. Relation metadata remains authored as human-readable IDs by
operation output, and relation mutation writes the target item's human ID even
when the caller supplied its MID.
:::

:::mara design DES-DOCUMENT-FORMAT
:mid: 01M1PXP2KG381MM1VNN6XC7S4M
:title: Mara document format
:status: accepted
:satisfies: REQ-CANONICAL-SOURCE
:kind: data

This contract defines canonical source syntax and its lossless in-memory projection.

## Documents

- A Mara document is a UTF-8 Markdown file named `*.mara.md`.
- Markdown outside Mara item blocks remains ordinary document content.
- Mara syntax inside fenced or inline code is example text, not project data.

## Items

An item has an explicit namespace, flavour, and complete human-readable ID:

```markdown
:::mara requirement REQ-FAIL-SAFETY
:mid: 01M1PXP2KGVW5ZF2JGP9K4XE9B
:title: Fail safely and observably
:depends_on: REQ-ACTIONABLE-DIAGNOSTICS

Failures preserve user data and produce actionable diagnostics.
:::
```

- The opening line is `:::mara <flavour> <id>` at the start of a line, with no
  other tokens.
- A flavour matches `[a-z][a-z0-9]*(?:_[a-z0-9]+)*`.
- An ID matches `[A-Z][A-Z0-9]*(?:-[A-Z0-9]+)+`, is mandatory and unique in
  the repository, and includes its human-meaningful prefix: `REQ-1`, not `1`.
- Flavours and ID prefixes follow the project taxonomy.
- IDs are current human-readable handles. Renaming changes that handle;
  the immutable MID remains the stable machine identity.
- An item closes with an exact standalone `:::`. Items cannot nest.

## Metadata and body

- Metadata is the contiguous sequence of `:key: value` lines after the opener.
- Keys use lowercase snake case. Values are single-line scalars whose
  surrounding whitespace is not semantic.
- Every item has exactly one non-empty `title` entry.
- Repeated keys are preserved in source order.
- The first blank line begins the Markdown body. Metadata-shaped lines after
  that boundary remain body content.
- The body may be empty.

## Machine identity

- `mid` is reserved structural metadata, not a configurable field.
- Every item has exactly one Mara-generated MID on the line immediately after
  its opener.
- A MID is a raw canonical uppercase 26-character ULID with no prefix.
- The MID is repository-wide unique and immutable for the item's lifetime.
- Callers never provide, copy, edit, or update MIDs through item operations.
- Do not create or copy placeholder MIDs by hand. Existing pre-alpha items
  receive MIDs through one deliberate backfill before identity-dependent
  editing is used.

## References and relations

- `[[REQ-FAIL-SAFETY]]` and `[[<MID>]]` are internal mentions resolved by
  exact human ID or canonical MID. Supported mentions occur in item bodies and
  narrative outside items, excluding Markdown code and raw contexts; escaped
  openings are literal text. Metadata scalar values are not mention sources.

- Typed relations use metadata `:relation: target` or item-body
  `[[relation:target]]` as declared by the project schema. Repeat either form
  for multiple targets.
- Use only relation names whose meaning is defined by the project corpus.
- Item relation targets resolve human IDs or MIDs to item MIDs; code and external
  targets retain their exact kind-specific identities. A declared inverse alias
  may author the same edge from its opposite endpoint; symmetric declarations have no
  semantic direction. Repeated assertions retain source occurrences but count
  as one edge. Reverse navigation never generates a second stored assertion.
- Bare inline references remain mentions. Typed inline tokens produce only a
  typed occurrence; they do not also produce a mention.

## In-memory projection

- Mara discovers project-relative `*.mara.md` files through the configured
  content include patterns and reads those canonical files directly.
- Documents are ordered by project-relative path and items remain in source
  order so repeated loads of unchanged files produce the same model.
- Each item retains its ordered metadata, exact Markdown body, schema-defined
  typed relations, body mentions outside fenced and inline code, and source
  locations.
- Valid item bodies retain Markdown children and their original source spans.
  Malformed metadata or incomplete item structure yields no body-block projection
  for that item while validation retains recoverable item data.
- A source location contains the project-relative path, an end-exclusive UTF-8
  byte span, and one-based start and end lines.
- The in-memory model is a disposable projection, never an authoring authority.

## Evolution

The document syntax has no embedded schema or project configuration. Introduce
validation or new syntax only for a demonstrated workflow. Persisted format
versions remain independent from the Mara application version.
:::

:::mara decision ADR-RUSHDOWN-PARSER-ADAPTER
:mid: 01M1PXP2KGG86FFPSNS8QWEXRZ
:title: Use Rushdown behind a Mara-owned Markdown adapter
:status: accepted
:justifies: DES-DOCUMENT-FORMAT

Use Rushdown custom block and inline extensions to recognize Mara structures with Markdown-aware code and raw-context handling and exact source spans. A private adapter converts the Rushdown result immediately into Mara-owned values, containing third-party AST and parser API churn behind that boundary.

Mara-owned source and block projections form the public contract under [[DES-DOCUMENT-FORMAT]] and [[DES-DOCUMENT-STRUCTURE]]; the parser AST remains private.
:::

:::mara verification VER-SOURCE-AND-IDENTITY
:mid: 01M3FWCR1XBCA886JKQAR163QH
:title: Check canonical parsing and durable identities
:status: accepted
:method: test
:level: system
:verifies: REQ-CANONICAL-SOURCE
:verifies: REQ-DURABLE-ITEM-IDENTITY
:verifies: REQ-MID-BACKFILL
:verifies: DES-DOCUMENT-FORMAT
:verifies: DES-DURABLE-ITEM-IDENTITIES

Run `cargo test --locked --test corpus --test identity` and the Markdown-container unit test against the candidate. Parser checks exercise document selection and ordering, item syntax, recovery, Markdown contexts, exact source spans and body preservation. The explicit read-only self-hosting check loads the real corpus twice without writing it; mutation fixtures use isolated temporary projects.

Real CLI/MCP checks reject malformed, missing, duplicate and misplaced identities, resolve a stable MID, and deliberately backfill legacy items. Reads must not backfill; successful backfill preserves pre-existing bytes and identities except for new MID lines; a rejected preflight changes nothing, including when user-controlled field text resembles a missing-MID diagnostic. Repeating backfill changes nothing.

Pass only when these processes and parser checks meet their assertions. Rename/move identity preservation is also checked by the mutation capability's own tests; this method does not claim that pending review.
:::

:::mara evidence EVD-SOURCE-AND-IDENTITY
:mid: 01M3FWPHJF68AGEWWKRZGNNZ8V
:title: Source and identity checkpoint passes restored suites
:status: accepted
:result: passed
:captured_at: 2026-09-26T22:16:47Z
:subject_revision: 35a37fc2bddaec0ef5943322bab7ce3fd65a321a
:evidences: VER-SOURCE-AND-IDENTITY
:evidences: VER-PROJECT-BOOTSTRAP

The tested working tree was committed unchanged as `35a37fc2bddaec0ef5943322bab7ce3fd65a321a`; Git reported a clean tree before this evidence was added. Linux x86_64, Rust 1.97.1, candidate `/tmp/mara72-target/debug/mara`; build settings `CARGO_TARGET_DIR=/tmp/mara72-target CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.

Passed `cargo fmt --all -- --check`, `cargo clippy --locked --all-targets --all-features -- -D warnings`, and `cargo test --locked --all-targets`: 58 tests passed (1 Markdown-container unit, 30 corpus, 9 identity/backfill, 18 bootstrap), none failed or ignored. Real CLI and stdio MCP checks used isolated temporary projects; the named deterministic corpus test intentionally performed read-only self-hosting.

Candidate schema/project validation returned complete, valid results with zero diagnostics. Explicit MCP matrices passed for 3 requirement origins and 5 requirement/design realization and verification roots, consuming all pages. Backfill checks confirmed no writes during reads, unchanged existing MIDs, exact non-MID source preservation, idempotence, and source preservation after rejected validation. Duplicate-identity assertions used stable classification and actual source locations because ambiguous item identities are intentionally omitted from those diagnostic records.

The result covers the source/identity and bootstrap methods on this candidate. Reference-resolution, schema-constraint, mutation-recovery and other remaining baseline reviews are not claimed complete.
:::
