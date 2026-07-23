# Exploration and indexing

Mara's first read interface is the CLI. It must answer direct questions about
the current working tree without a database, while also producing a stable JSON
projection that later interfaces can consume or replace with another derived
backend.

## Common query behaviour

:::req m_01KY7YA2CNMH2B11VJN1V9RCQY
:id: REQ-QUERY-RESOLUTION
:title: Query commands shall resolve MID or display ID
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: SCN-EXPLORE-ITEM

Any command that accepts an item reference shall apply the same exact MID-then-
display-ID resolution contract as validation and shall report unresolved or
ambiguous input without heuristic matching.
:::

:::req m_01KY7YA2CPKFBF7ZHDMSPD7VNB
:id: REQ-QUERY-FORMATS
:title: Query commands shall provide human and JSON output
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-NAVIGATE-TRACE
:derives_from: GOAL-AGENT-READY

`list`, `show`, and `trace` shall support `--format human|json`. Human output
shall optimize terminal reading; JSON output shall expose complete stable data
without scraping terminal presentation.
:::

## Item listing

:::req m_01KY7YA2CQ5ZDY4TPSF0Z0RC3S
:id: REQ-LIST-ITEMS
:title: mara list shall enumerate project items deterministically
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

`mara list` shall enumerate items in a stable order and show at least MID,
display ID when present, flavour, title, and project-relative source location.
:::

:::req m_01KY7YA2CR1DSJYV2X25F9PMF4
:id: REQ-LIST-FILTERS
:title: mara list shall filter by flavour and field values
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: STORY-NAVIGATE-TRACE

The list command shall accept repeated flavour filters and exact scalar field
filters. Values within one flavour or field filter shall combine with OR;
different field names and the flavour constraint shall combine with AND. The
JSON result shall record normalized effective filters even when they are empty.
A repeatable field shall match when any authored value equals any compiled filter
value for the item's flavour; absent fields shall not match.
:::

## Item inspection

:::req m_01KY7YA2CSXP38N88WWVN65SN9
:id: REQ-SHOW-ITEM
:title: mara show shall expose the complete resolved item
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-EXPLORE-ITEM

`mara show <ref>` shall display structural identity, built-ins, typed fields,
body, document placement, source spans, authored relation occurrences,
canonical outgoing and incoming relations, weak mentions, backlinks, and
per-occurrence provenance.
:::

## Trace traversal

:::req m_01KY7YA2CT9Z63V0KZR746BG4Z
:id: REQ-TRACE-DIRECTION
:title: mara trace shall traverse selected edge directions
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: SCN-EXPLORE-ITEM

`mara trace <ref>` shall support incoming, outgoing, and bidirectional traversal
over canonical typed edges, with explicit identification of the direction and
relation used for each result.
:::

:::req m_01KY7YA2CVQYV5XFGPPD1JVC9C
:id: REQ-TRACE-DEPTH
:title: Trace traversal shall be bounded and cycle-safe
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: STORY-NAVIGATE-TRACE

Trace shall default to direct neighbours and accept a bounded positive depth.
It shall return every distinct simple canonical-edge path from the focus with one
through the selected maximum number of steps, never repeat a MID within a path,
exclude the zero-step focus path, and order nodes and paths deterministically.
Different canonical relation names between the same endpoints are distinct paths;
repeated authored occurrences of one canonical edge are not.
:::

## Deterministic JSON projection

:::req m_01KY7YA2CWWZ6K78Q7ERE0F0S5
:id: REQ-INDEX-COMMAND
:title: mara index shall write a rebuildable JSON projection
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-GIT-CANONICAL
:uses_term: TERM-DERIVED-PROJECTION

`mara index` shall validate the project and write the configured JSON index only
when no error prevents a coherent normalized model. The index shall be a
replaceable generated file and never an authoring input.
:::

:::req m_01KY7YA2CX018G0FHR43XYW4XM
:id: REQ-INDEX-CONTENT
:title: The JSON index shall contain the complete normalized project model
:status: approved
:level: system
:kind: interface
:priority: must
:derives_from: STORY-NAVIGATE-TRACE
:derives_from: GOAL-AGENT-READY

The index shall contain project and schema identity, documents, sections,
narrative blocks, item placements, raw bodies, typed fields, canonical MID
edges, inverse presentation, weak mentions, external nodes, source spans,
relation provenance, and available Git provenance.
:::

:::req m_01KY7YA2CYFMXA5JXG2PV7ADB8
:id: REQ-INDEX-DETERMINISTIC
:title: The JSON index shall be byte-deterministic for equivalent input
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-AUDITABLE

Mara shall define stable ordering and serialization for every index collection
and object. It shall omit timestamps, absolute machine paths, random values, and
other host-specific data that would change bytes for equivalent project input
and equivalent relevant Git state.
:::

:::req m_01KY7YA2CZG8TYWC1M77KDA3N2
:id: REQ-INDEX-GIT-PROVENANCE
:title: The index shall record available relevant Git provenance
:status: approved
:level: system
:kind: functional
:priority: should
:derives_from: GOAL-AUDITABLE

Inside a worktree, the index shall record HEAD commit, branch when available,
the project path relative to repository root, and whether relevant project files
are modified or untracked. It shall not embed the host's absolute repository
path. Outside Git these fields shall be explicitly unavailable.
:::

:::req m_01KY7YA2D044X5TE8Z9JYNC5NQ
:id: REQ-INDEX-FULL-REBUILD
:title: v0.1 shall rebuild the complete model on every command
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

v0.1 query, validation, and index commands shall parse and normalize the full
selected project without persistent caches, file hashes, Git-diff shortcuts, or
incremental graph mutation. Caching shall require later profiling and shall not
change semantic results.
:::

## Portability and scale

:::req m_01KY7YA2D15KMVGMMMQ9NZ5B5S
:id: REQ-PORTABLE-CLI
:title: Mara v0.1 shall support Linux, macOS, and Windows
:status: approved
:level: system
:kind: quality
:priority: should
:derives_from: GOAL-SCALABLE

The CLI and reusable libraries shall support current stable Rust on Linux,
macOS, and Windows with equivalent path normalization, document semantics,
diagnostic codes, and normalized JSON output.
:::

:::req m_01KY7YA2D2VJ961QCVMGTNNK21
:id: REQ-PERFORMANCE-TARGET
:title: Full validation shall meet the v0.1 scale budget
:status: approved
:level: system
:kind: performance
:priority: should
:derives_from: GOAL-SCALABLE

On a documented CI reference runner, a clean full check of 10,000 items and
100,000 resolved edges shall complete within five seconds and use no more than
512 MiB of resident memory.
:::

## Design, rationale, risk, and artifacts

:::design m_01KY7YA2D74S6NC0FGAZ2ZKFT2
:id: DES-QUERY-INDEX
:title: Shared in-memory project model with projection adapters
:status: accepted
:kind: architecture
:satisfies: REQ-QUERY-RESOLUTION
:satisfies: REQ-QUERY-FORMATS
:satisfies: REQ-LIST-ITEMS
:satisfies: REQ-LIST-FILTERS
:satisfies: REQ-SHOW-ITEM
:satisfies: REQ-TRACE-DIRECTION
:satisfies: REQ-TRACE-DEPTH
:satisfies: REQ-INDEX-COMMAND
:satisfies: REQ-INDEX-CONTENT
:satisfies: REQ-INDEX-DETERMINISTIC
:satisfies: REQ-INDEX-GIT-PROVENANCE
:satisfies: REQ-INDEX-FULL-REBUILD
:satisfies: REQ-PORTABLE-CLI
:satisfies: REQ-PERFORMANCE-TARGET

All read commands shall consume one immutable normalized project model. Human
formatters, JSON command output, and the index writer are presentation adapters
over that model, preventing CLI-specific semantics from diverging from future
library and web consumers.
:::

:::decision m_01KY7YA2D87T12812J64ZMN1GF
:id: ADR-0007
:title: Rebuild before introducing index complexity
:status: accepted
:kind: architecture
:justifies: DES-QUERY-INDEX
:justifies: REQ-INDEX-FULL-REBUILD

A complete deterministic rebuild is simple to reason about, verifies that Git
source is sufficient, and provides a correctness reference for future caching,
SQLite, or graph projections. Performance optimization follows measured need.
:::

:::risk m_01KY7YA2D9S1Y8RXZTQYPQMGFN
:id: RISK-INDEX-AUTHORITY
:title: Consumers may mistake a generated index for authoritative data
:status: open
:severity: high
:likelihood: medium
:affects: REQ-INDEX-COMMAND
:affects: REQ-INDEX-CONTENT

Fast consumers may be tempted to mutate or preserve an index independently of
source. Every backend and interface must expose its source revision and retain a
clear rebuild path from Git-tracked files.
:::

:::artifact m_01KY7YA2DANZ8F7690GGWEM9TS
:id: ART-MARA-CLI
:title: Mara command-line interface
:status: proposed
:kind: command
:uri: crates/mara-cli

The CLI exposes initialization, MID generation, validation, inspection,
traversal, indexing, and display-ID editing over reusable Mara libraries.
:::

:::artifact m_01KY7YA2DBYSDJ0MD0CTJAP23Z
:id: ART-JSON-PROJECTION
:title: Versioned JSON projection format
:status: proposed
:kind: file_format
:uri: .mara/index.json

The deterministic JSON format is the first complete machine-facing projection
of a normalized Mara project.
:::

## Planned verification

:::test m_01KY7YA2D31YRA5J3FVWDZPDCM
:id: TEST-LIST-SHOW
:title: Item listing and inspection acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-QUERY-RESOLUTION
:verifies: REQ-QUERY-FORMATS
:verifies: REQ-LIST-ITEMS
:verifies: REQ-LIST-FILTERS
:verifies: REQ-SHOW-ITEM

CLI fixtures shall cover MID and display-ID resolution, deterministic listing,
OR within repeated flavour and field values, AND across field names, normalized
effective filter JSON including empty and cross-flavour mixed-type filters,
unknown and unconvertible filter errors, repeatable-field existential matching,
absent fields, duplicate values, numeric negative-zero equality, self-contained
show mentions, human snapshots, and stable JSON data.
:::

:::test m_01KY7YA2D4ZH3EBGK1FN9BBBYC
:id: TEST-TRACE
:title: Bounded trace traversal acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-TRACE-DIRECTION
:verifies: REQ-TRACE-DEPTH

Graph fixtures containing branches, cycles, inverse authoring, and repeated
paths shall verify direction, depth, path reporting, cycle safety, and stable
ordering. Every JSON path step shall expose canonical source and target plus the
actual incoming or outgoing traversal direction. Exact golden path sets shall
cover a directed cycle, a diamond, and two different canonical relations between
the same endpoints; they shall prove simple-path exclusion of repeated MIDs,
absence of the zero-step focus path, occurrence deduplication, and relation-based
parallel-path preservation.
:::

:::test m_01KY7YA2D55K5KDK24KXTBWJVF
:id: TEST-JSON-INDEX
:title: Deterministic JSON projection acceptance test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-INDEX-COMMAND
:verifies: REQ-INDEX-CONTENT
:verifies: REQ-INDEX-DETERMINISTIC
:verifies: REQ-INDEX-GIT-PROVENANCE
:verifies: REQ-INDEX-FULL-REBUILD

Golden indexes shall cover complete document and graph content, clean and dirty
Git states, unversioned directories, repeated rebuilds, changed filesystem
enumeration order, exact v1 key and collection ordering, null policy, canonical
UTF-8 serialization, and absence of machine-specific absolute paths.
:::

:::test m_01KY7YA2D6CG9DXA8CBGGV3B5G
:id: TEST-PORTABILITY-PERFORMANCE
:title: Cross-platform and scale qualification test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-PORTABLE-CLI
:verifies: REQ-PERFORMANCE-TARGET

CI shall run behavioural suites on Linux, macOS, and Windows and shall benchmark
the documented 10,000-item, 100,000-edge fixture on the designated reference
runner against time and memory budgets.
:::
