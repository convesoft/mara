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
every item that lacks one. Backfill preserves existing source content and
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
- Do not create or copy placeholder MIDs by hand. Items lacking a MID receive
  one through deliberate backfill under [[REQ-MID-BACKFILL]] before
  identity-dependent editing is used.

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
  content include patterns and reads those canonical files directly. Discovery
  respects local and parent Git ignore rules and does not follow document or
  directory symlinks.
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

Document loading produces a document-only snapshot through `load_documents` and
recovering variants. Strict loading rejects malformed or unreadable source;
recovery retains independently available data and diagnostics with explicit
completeness. It does not run code discovery or semantic conformance checks.
Full corpus operations compose this dependency with code discovery before
retrieval; a document-only result is not a substitute for that full read. These
library boundaries do not introduce new CLI/MCP commands.

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

Run `cargo test --locked --test corpus --test corpus_validation --test mid_recovery` and the Markdown-container unit test against the candidate. Parser checks exercise document selection and ordering, item syntax, recovery, Markdown contexts, exact source spans and body preservation. The explicit read-only self-hosting check loads the real corpus twice without writing it; mutation fixtures use isolated temporary projects.

Real CLI/MCP checks reject malformed, missing, duplicate and misplaced identities, resolve a stable MID, and deliberately backfill items without MIDs. Reads must not backfill; successful backfill preserves pre-existing bytes and identities except for new MID lines; a rejected preflight changes nothing, including when user-controlled field text resembles a missing-MID diagnostic. Repeating backfill changes nothing.

Pass only when these processes and parser checks meet their assertions. [[VER-ITEM-RENAME]] and [[VER-ITEM-MOVEMENT]] check identity preservation across their respective mutations.
:::

:::mara verification VER-DOCUMENT-PARSING
:mid: 01M3FZHR3XW51YZK11QXDYXC9J
:title: Load canonical document sources without code or graph operations
:status: accepted
:method: test
:level: integration
:verifies: REQ-CANONICAL-SOURCE
:verifies: DES-DOCUMENT-FORMAT

Run `cargo test --locked --lib --test corpus` against the candidate library. Use isolated temporary projects with their own Git/configuration state, plus the explicitly named read-only repository test. Exercise configured discovery and stable ordering; local/parent Git ignores; item, metadata and body parsing; malformed structure and recovery; code/raw/escaped Markdown contexts; container/table/reference spans; and exact UTF-8/CRLF source preservation.

Require strict document loading to reject malformed/unreadable source. Recovery retains independently readable documents and available diagnostics, marks incomplete discovery/parsing, and does not invent coordinates for unreadable bytes. Verify no read backfills MIDs or changes source. Keep the private-parser unit test that checks item containers own only Markdown body children.

Check that `load_documents` and its recovering variants return document-only snapshots without loading code integrations. A fixture with unavailable code assets distinguishes this boundary from full corpus loading. [[VER-CODE-DISCOVERY]], [[VER-CORPUS-CONFORMANCE]] and [[VER-DOCUMENT-NAVIGATION]] own code loading, semantic conformance and navigation checks. Run the CLI/MCP regression suite alongside these library checks.
:::
