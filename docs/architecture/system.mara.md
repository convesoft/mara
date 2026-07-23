# System architecture

Mara is implemented as a reusable deterministic semantic engine with a thin CLI.
The architecture mirrors the product boundary: Markdown files and Git history are
canonical; parser ASTs, normalized graphs, indexes, reports, and interfaces are
replaceable representations.

## Architectural requirements

:::req m_01KY7YA2EDBK585SSHFPYKS69P
:id: REQ-ARCH-LIBRARY-FIRST
:title: Mara shall expose reusable library APIs beneath the CLI
:status: approved
:level: system
:kind: quality
:priority: must
:derives_from: GOAL-SCHEMA-GENERIC

All project loading, parsing, normalization, validation, querying, indexing, and
editing behaviour shall be available through Rust library APIs. The CLI shall
perform argument and presentation handling without becoming the only reusable
integration surface.
:::

:::req m_01KY7YA2EE47VQD2FCZ83N1Y00
:id: REQ-ARCH-DOMAIN-OWNERSHIP
:title: The domain model shall be owned by Mara
:status: approved
:level: system
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

Mara-owned types shall define projects, schemas, documents, items, values,
references, canonical relations, external nodes, diagnostics, provenance, and
source spans. Public semantic contracts shall not expose Rushdown AST node types,
Git library objects, CLI argument types, or storage-backend records.
:::

:::req m_01KY7YA2EF09XEPF2G7KAMEQA6
:id: REQ-ARCH-DOCUMENT-HIERARCHY
:title: The domain model shall retain complete document hierarchy
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-READABLE-SOURCE
:uses_term: TERM-DOCUMENT-MODEL

A loaded document shall contain ordered sections, narrative blocks, and item
placements with exact spans. Narrative Markdown shall retain raw source and a
renderable Mara-owned representation even when it has no MID or flavour, so the
CLI, index, later generated views, and web UI can present coherent documents
rather than only disconnected items.
:::

:::req m_01KY7YA2EG9DRTNAQG2WNB1T5F
:id: REQ-ARCH-PARSER-BOUNDARY
:title: Markdown parser output shall be converted at one boundary
:status: approved
:level: software
:kind: constraint
:priority: must
:derives_from: GOAL-AUDITABLE

The Markdown adapter shall extend Rushdown with Mara directive and inline-
reference nodes, preserve raw source spans, and immediately convert parser output
to Mara-owned parsed-document types. Later services shall not depend on the
parser's unstable internal tree or perform a second independent syntax parse.
:::

:::req m_01KY7YA2EHB5DVCV9EXKK85E9M
:id: REQ-ARCH-DETERMINISTIC-PIPELINE
:title: Project compilation shall use one ordered semantic pipeline
:status: approved
:level: system
:kind: functional
:priority: must
:derives_from: GOAL-AUDITABLE

The engine shall discover files, decode and parse documents, collect unresolved
items and references, validate schema-level structure, build identity indexes,
resolve internal and external targets, normalize canonical edges, derive inverse
views, evaluate project rules, and expose one immutable project result. Query and
projection commands shall consume that result rather than reimplementing stages.
:::

:::req m_01KY7YA2EJXBPPEDCD3VB07AZM
:id: REQ-ARCH-SOURCE-DERIVED-BOUNDARY
:title: Source and derived state shall remain distinguishable in every service
:status: approved
:level: system
:kind: safety
:priority: must
:derives_from: GOAL-GIT-CANONICAL
:uses_term: TERM-DERIVED-PROJECTION

Domain values shall retain whether they were authored, normalized from inverse
authoring, inferred as an inverse or symmetric view, extracted as a weak mention,
or supplied by a derived source scanner. Only explicit source occurrences may be
edited; a derived backlink or projection record shall never be mistaken for an
authorable second copy.
:::

:::req m_01KY7YA2EKME9XDW5V4G36SVFX
:id: REQ-ARCH-PURE-DOMAIN
:title: Core semantic types and algorithms shall not perform infrastructure I/O
:status: approved
:level: software
:kind: quality
:priority: should
:derives_from: GOAL-SCALABLE

Identity, values, relation normalization, traversal, diagnostics, and rule
evaluation shall be testable from in-memory inputs. Filesystem discovery, Git,
terminal output, and future databases or network adapters shall enter through
explicit engine ports around the semantic core.
:::

:::req m_01KY7YA2EMZBRR3XD3T05730VG
:id: REQ-ARCH-STRUCTURED-FAILURES
:title: Libraries shall return structured failures and diagnostics
:status: approved
:level: software
:kind: interface
:priority: must
:derives_from: GOAL-AGENT-READY

Recoverable project defects shall be returned as ordered diagnostics with stable
codes and source evidence. Operational failures shall use structured error types
with preserved causes and affected paths. Libraries shall not print or terminate
the process; the CLI shall map results to human or JSON output and exit status.
:::

## Workspace design

:::design m_01KY7YA2EN3TREXHMS28SHAE2Y
:id: DES-RUST-WORKSPACE
:title: Four-layer Rust workspace
:status: accepted
:kind: architecture
:satisfies: REQ-ARCH-LIBRARY-FIRST
:satisfies: REQ-ARCH-DOMAIN-OWNERSHIP
:satisfies: REQ-ARCH-DOCUMENT-HIERARCHY
:satisfies: REQ-ARCH-PARSER-BOUNDARY
:satisfies: REQ-ARCH-DETERMINISTIC-PIPELINE
:satisfies: REQ-ARCH-SOURCE-DERIVED-BOUNDARY
:satisfies: REQ-ARCH-PURE-DOMAIN
:satisfies: REQ-ARCH-STRUCTURED-FAILURES

The workspace shall separate the domain kernel, Markdown adapter, application
engine, and CLI. Dependency direction is CLI to engine, engine to domain and
Markdown, and Markdown to domain. The domain kernel has no dependency on higher
layers. Integration traits live at the layer that consumes them.
:::

:::decision m_01KY7YA2EPEBZ9RRZTNGF38TH9
:id: ADR-0010
:title: Use a library-first layered Rust workspace
:status: accepted
:kind: architecture
:justifies: DES-RUST-WORKSPACE
:justifies: REQ-ARCH-LIBRARY-FIRST
:justifies: REQ-ARCH-PARSER-BOUNDARY

Mara is expected to serve a CLI first and later a web service, graph projection,
source scanners, and agent integrations. Isolating semantic contracts prevents
those clients from depending on CLI behaviour or a particular Markdown and Git
library.
:::

:::risk m_01KY7YA2EQ40653NKBNDH3GBGT
:id: RISK-PARSER-COUPLING
:title: Parser AST leakage may freeze third-party implementation details
:status: open
:severity: high
:likelihood: medium
:affects: REQ-ARCH-DOMAIN-OWNERSHIP
:affects: REQ-ARCH-PARSER-BOUNDARY
:affects: DES-RUST-WORKSPACE

Rushdown extensions are useful for precise Markdown parsing, but exposing those
nodes beyond the adapter would make the domain and stored index inherit parser
changes. Boundary conversion and Mara-owned source representations contain that
risk.
:::

## Planned workspace artifacts

:::artifact m_01KY7YA2ERHCGW00ARN2NA9EBV
:id: ART-MARA-CORE
:title: mara-core crate
:status: proposed
:kind: crate
:uri: crates/mara-core
:implements: DES-RUST-WORKSPACE
:implements: REQ-ARCH-DOMAIN-OWNERSHIP
:implements: REQ-ARCH-PURE-DOMAIN

The domain crate owns semantic types, value and identity rules, normalized graph
operations, traversal, and diagnostics that require no infrastructure I/O.
:::

:::artifact m_01KY7YA2ES6FJCV9HN6JS3K9Z3
:id: ART-MARA-MARKDOWN
:title: mara-markdown crate
:status: proposed
:kind: crate
:uri: crates/mara-markdown
:implements: DES-RUST-WORKSPACE
:implements: REQ-ARCH-DOCUMENT-HIERARCHY
:implements: REQ-ARCH-PARSER-BOUNDARY

The Markdown crate integrates Rushdown, recognizes Mara syntax and inline
references, and converts complete source documents into Mara-owned parsed forms.
:::

:::artifact m_01KY7YA2ETD33W9H6JWM5DPPBK
:id: ART-MARA-ENGINE
:title: mara-engine crate
:status: proposed
:kind: crate
:uri: crates/mara-engine
:implements: DES-RUST-WORKSPACE
:implements: REQ-ARCH-DETERMINISTIC-PIPELINE
:implements: REQ-ARCH-SOURCE-DERIVED-BOUNDARY

The engine crate owns project discovery, configuration, schema loading, semantic
compilation, validation orchestration, query services, projection adapters, Git
provenance, and transactional source editing.
:::

:::artifact m_01KY7YA2EVZ3XSRF412A83KJ12
:id: ART-MARA-CLI-CRATE
:title: mara-cli crate and mara executable
:status: proposed
:kind: crate
:uri: crates/mara-cli
:implements: DES-RUST-WORKSPACE
:implements: REQ-ARCH-LIBRARY-FIRST
:implements: REQ-ARCH-STRUCTURED-FAILURES

The CLI crate defines commands, input policy, human and JSON presentation, and
process exit mapping while delegating semantic operations to the engine API.
:::

## Planned architecture verification

:::test m_01KY7YA2EWCWA35N19X317C7MR
:id: TEST-ARCH-DOMAIN-BOUNDARIES
:title: Domain ownership and dependency boundary test
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: REQ-ARCH-LIBRARY-FIRST
:verifies: REQ-ARCH-DOMAIN-OWNERSHIP
:verifies: REQ-ARCH-DOCUMENT-HIERARCHY
:verifies: REQ-ARCH-PARSER-BOUNDARY
:verifies: REQ-ARCH-PURE-DOMAIN
:verifies: REQ-ARCH-STRUCTURED-FAILURES

Architecture checks shall reject reverse crate dependencies and parser, CLI, or
infrastructure types in public domain APIs. Fixtures shall prove that complete
documents and structured failures are available through libraries without a CLI.
:::

:::test m_01KY7YA2EX6CJXJRG416X4NB4C
:id: TEST-ARCH-PIPELINE
:title: Deterministic compilation pipeline test
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-ARCH-DETERMINISTIC-PIPELINE
:verifies: REQ-ARCH-SOURCE-DERIVED-BOUNDARY

Repeated compilation shall produce the same project result and diagnostics.
Fixtures shall distinguish authored edges, inverse authoring, inferred backlinks,
weak mentions, external targets, and derived source spans through all query and
projection interfaces.
:::
