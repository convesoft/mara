# Authoring language

Mara documents are Markdown documents with a small, versioned structural
extension for typed items and wiki-style references. Unsupported renderers may
show Mara markers as ordinary text, but they shall not turn complete items into
opaque code blocks or destroy the readable narrative.

## Item blocks

The canonical form keeps immutable identity visually close to the item boundary:

```markdown
:::req m_01JX0TV1P2V1N0VJ3M3J6W9Y7R
:id: REQ-EXAMPLE-001
:title: Example requirement
:status: approved

Mara shall preserve readable Markdown bodies.
:::
```

:::req m_01KY7Y9R54VT602R7E0HXCN7BQ
:id: REQ-LANGUAGE-ITEM-BLOCK
:title: Mara shall use direct-flavour colon item blocks
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

An item shall open with `:::<flavour> <mid>` at the beginning of a line and
close with a standalone `:::` at the beginning of a line. The parser shall treat
the flavour as schema-defined data and the MID as structural identity.
:::

:::req m_01KY7Y9R55KHF4NK76KWWR4D6N
:id: REQ-LANGUAGE-METADATA-BOUNDARY
:title: Mara shall separate metadata from body with one blank line
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Metadata shall be the contiguous sequence of `:key: value` lines immediately
after the opening line. The first blank line shall permanently begin the body;
metadata-shaped text after that boundary shall remain body content.
:::

:::req m_01KY7Y9R564VFBWCQSCN24K9GR
:id: REQ-LANGUAGE-METADATA-VALUES
:title: Mara shall parse one scalar value per metadata line
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Metadata keys shall be lowercase ASCII snake case and case-sensitive. The raw
text after the key delimiter shall form one scalar value; semantic surrounding
whitespace shall be trimmed while the exact source text and span are preserved.
Multiline and nested metadata values shall not be supported in v0.1.
:::

:::req m_01KY7Y9R57JR2TQMQQBS0B5GGR
:id: REQ-LANGUAGE-REPEATED-KEYS
:title: Mara shall preserve repeated metadata entries
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Repeated metadata keys shall remain independent ordered entries. Schema
resolution shall determine whether repetition is valid, allowing list-like
fields and one-relation-target-per-line diffs without comma-separated syntax.
:::

:::req m_01KY7Y9R58RB71PZYJ2RD70XJB
:id: REQ-LANGUAGE-BODY
:title: Mara shall preserve an item's Markdown body
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE

Everything after the metadata boundary and before the closing marker shall be
the item's Markdown body. The platform shall permit an empty body while the
flavour schema determines whether body content is required.
:::

:::req m_01KY7Y9R59ZRJCNX322FA5AS0J
:id: REQ-LANGUAGE-TOP-LEVEL
:title: Mara item blocks shall be top-level document blocks
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-READABLE-SOURCE

Mara shall not nest authored item blocks. Mara-like text inside fenced code,
inline code, raw HTML, or an existing item body shall not open another item. A
closing marker is recognized only when the source line, excluding its LF or CRLF
terminator, is exactly `:::` with no leading or trailing whitespace. To place
that rendered text on its own body line, the author shall write `\:::` at byte
column zero. The backslash remains in the raw body and source spans, does not
close the item, and is interpreted by CommonMark as an escape so the rendered
line is `:::`. To render a leading backslash followed by three colons, the author
shall write `\\:::`. Fenced code remains the preferred form for multi-line syntax
examples.
:::

## Complete document model

Narrative is part of the product, not parse-time whitespace around extracted
items. It must remain available to renderers, editors, and context assembly.

:::req m_01KY7Y9R5A2RKWNHYW5VKNJFCK
:id: REQ-LANGUAGE-DOCUMENT-MODEL
:title: Mara shall model complete source documents
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-READABLE-SOURCE
:uses_term: TERM-DOCUMENT-MODEL

The parsed project shall retain each source document as an ordered hierarchy of
sections, narrative blocks, and item placements. Extracting items shall not
discard headings, ordinary Markdown, or the position of an item within its
document.
:::

:::req m_01KY7Y9R5BNM52P8Z4TGCRM6VA
:id: REQ-LANGUAGE-NARRATIVE
:title: Mara shall retain narrative as first-class document content
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-AGENT-READY
:derives_from: GOAL-READABLE-SOURCE

Each narrative block shall retain raw Markdown, parsed Markdown structure, and
its source span. Narrative shall not require an MID or flavour. Authors may
promote independently traceable prose to a schema-defined item when it needs
identity, lifecycle, or semantic relations.
:::

:::req m_01KY7Y9R5CZ6BMDSVZ78SKF502
:id: REQ-LANGUAGE-NARRATIVE-MENTIONS
:title: Mara shall index bare references in ordinary narrative
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-NAVIGATE-TRACE

A bare inline reference in ordinary narrative shall create a navigable derived
mention whose source is the narrative span. A typed relation outside an authored
item shall be invalid because no semantic source item exists.
:::

## Inline-reference contexts

:::req m_01KY7Y9R5DQSB7GVN2YYCD3PX4
:id: REQ-LANGUAGE-INLINE-CONTEXT
:title: Mara shall parse inline references only in Markdown text contexts
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-AUTHOR-KNOWLEDGE
:uses_term: TERM-INLINE-REFERENCE

Mara shall recognize inline references in ordinary text nodes, headings, list
items, and table cells. It shall ignore reference-shaped content in inline code,
fenced code, and raw HTML.
:::

:::req m_01KY7Y9R5EAKM5RJWWRNS6KB1D
:id: REQ-LANGUAGE-INLINE-ESCAPE
:title: Mara shall support escaped literal inline references
:status: approved
:level: system
:kind: interface
:priority: should
:derives_from: STORY-AUTHOR-KNOWLEDGE

An escaped opening such as `\[[REQ-001]]` shall render as literal reference text
and shall not create a mention or relation.
:::

## Encoding, spans, and recovery

:::req m_01KY7Y9R5F75AMQH7A81R0FB88
:id: REQ-LANGUAGE-UTF8
:title: Mara shall accept UTF-8 documents with LF or CRLF endings
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: GOAL-READABLE-SOURCE

Source documents shall be valid UTF-8. Mara shall accept LF and CRLF line
endings, record the existing style, preserve it during edits, and report invalid
UTF-8 as a file diagnostic.
:::

:::req m_01KY7Y9R5GFF6MKHEJA4337KXQ
:id: REQ-LANGUAGE-SOURCE-SPANS
:title: Mara shall preserve exact source spans
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: STORY-RENAME-DISPLAY-ID
:derives_from: STORY-WEB-AUTHORING

The parser shall record UTF-8 byte and line spans for each document, section,
narrative block, item, opening line, metadata entry, body, and inline reference.
Spans shall support precise diagnostics and minimal future source patches.
:::

:::req m_01KY7Y9R5H5QWDEV3Y0BXRW5AZ
:id: REQ-LANGUAGE-PARSE-RECOVERY
:title: Mara shall contain malformed-document failures
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: STORY-VALIDATE-PROJECT

A malformed item shall produce a precise parse diagnostic. When boundaries are
ambiguous Mara may stop extracting further items from that file, but it shall
continue checking every other independently parseable project file.
:::

## Design, rationale, risk, and artifact

:::design m_01KY7Y9R5N16ABXCG218MWYFEV
:id: DES-DOCUMENT-PARSER
:title: Markdown AST with Mara structural nodes
:status: accepted
:kind: component
:satisfies: REQ-LANGUAGE-ITEM-BLOCK
:satisfies: REQ-LANGUAGE-METADATA-BOUNDARY
:satisfies: REQ-LANGUAGE-METADATA-VALUES
:satisfies: REQ-LANGUAGE-REPEATED-KEYS
:satisfies: REQ-LANGUAGE-BODY
:satisfies: REQ-LANGUAGE-TOP-LEVEL
:satisfies: REQ-LANGUAGE-DOCUMENT-MODEL
:satisfies: REQ-LANGUAGE-NARRATIVE
:satisfies: REQ-LANGUAGE-NARRATIVE-MENTIONS
:satisfies: REQ-LANGUAGE-INLINE-CONTEXT
:satisfies: REQ-LANGUAGE-INLINE-ESCAPE
:satisfies: REQ-LANGUAGE-UTF8
:satisfies: REQ-LANGUAGE-SOURCE-SPANS
:satisfies: REQ-LANGUAGE-PARSE-RECOVERY

The Rust parser shall extend Rushdown with Mara block and inline nodes, retain
the complete Markdown AST and raw source spans, and immediately convert parsed
constructs into Mara-owned document and item models. Rushdown's AST shall not be
the durable domain or index contract.
:::

:::decision m_01KY7Y9R5PJY27P7YNS2ZZ61CQ
:id: ADR-0003
:title: Use direct flavour names in colon block headers
:status: accepted
:kind: syntax
:justifies: DES-DOCUMENT-PARSER
:justifies: REQ-LANGUAGE-ITEM-BLOCK

The mandatory MID-shaped second token distinguishes Mara items from ordinary
colon containers, so `:::req <mid>` remains generic without the extra visual
noise of `:::item req <mid>`.
:::

:::risk m_01KY7Y9R5Q647RMBJFPWVXSBWQ
:id: RISK-MARKDOWN-AMBIGUITY
:title: Mara markers may conflict with Markdown extension syntax
:status: open
:severity: medium
:likelihood: medium
:affects: DES-DOCUMENT-PARSER
:affects: REQ-LANGUAGE-TOP-LEVEL

Other Markdown systems may also interpret colon containers or wiki links. Mara
must use the complete opening grammar and Markdown context rather than raw text
matching, and `.mara.md` remains the default discovery convention.
:::

:::artifact m_01KY7Y9R5RGT9P925P7YA3Q3SR
:id: ART-MARA-LANGUAGE
:title: Mara Markdown language surface
:status: proposed
:kind: file_format
:uri: docs/**/*.mara.md

Mara Markdown combines ordinary Markdown documents, direct-flavour item blocks,
flat metadata entries, and wiki-style inline references.
:::

## Planned verification

:::test m_01KY7Y9R5JV5C403GQ6ESFG1CN
:id: TEST-LANGUAGE-BLOCKS
:title: Item block grammar test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-LANGUAGE-ITEM-BLOCK
:verifies: REQ-LANGUAGE-METADATA-BOUNDARY
:verifies: REQ-LANGUAGE-METADATA-VALUES
:verifies: REQ-LANGUAGE-REPEATED-KEYS
:verifies: REQ-LANGUAGE-BODY
:verifies: REQ-LANGUAGE-TOP-LEVEL

Golden fixtures shall cover minimal and populated items, repeated keys, blank
metadata values, body metadata lookalikes, nested-marker rejection, code-fenced
examples, exact `:::` closers, `\:::` rendered closers, `\\:::` literal escaped
closers, leading or trailing whitespace, and missing closing markers.
:::

:::test m_01KY7Y9R5K6SW0Z2PMG0CHH8V8
:id: TEST-LANGUAGE-DOCUMENT
:title: Complete document model test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-LANGUAGE-DOCUMENT-MODEL
:verifies: REQ-LANGUAGE-NARRATIVE
:verifies: REQ-LANGUAGE-NARRATIVE-MENTIONS

Fixtures shall assert ordered sections, narrative blocks, item placements,
source-preserved Markdown, and derived narrative-span mentions.
:::

:::test m_01KY7Y9R5MAVA48BHF0NHRVDDR
:id: TEST-LANGUAGE-INLINE
:title: Markdown-context inline reference test
:status: approved
:kind: verification
:method: automated
:level: unit
:verifies: REQ-LANGUAGE-INLINE-CONTEXT
:verifies: REQ-LANGUAGE-INLINE-ESCAPE

Fixtures shall place reference-shaped text in paragraphs, headings, lists,
tables, code spans, fenced code, raw HTML, and escaped text and assert exactly
which references are extracted.
:::

:::test m_01KY7Y9R5SMJPZ2A3HN634RZDN
:id: TEST-LANGUAGE-PROVENANCE
:title: Encoding, source span, and recovery test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-LANGUAGE-UTF8
:verifies: REQ-LANGUAGE-SOURCE-SPANS
:verifies: REQ-LANGUAGE-PARSE-RECOVERY

Fixtures shall cover Unicode text, LF, CRLF, invalid UTF-8, exact byte and line
spans, malformed files, and continued diagnostics from unaffected files.
:::
