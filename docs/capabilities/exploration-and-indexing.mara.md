# Exploration and indexing

Mara's first read interface is a CLI over the current working tree. It supports
direct inspection without a database and can produce a stable JSON projection
for later derived interfaces.

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
through the selected maximum number of steps, never repeat an item, derived
source-span, or external-node identity within a path, exclude the zero-step focus
path, and order nodes and paths deterministically. Different canonical relation
names between the same endpoints are distinct paths; repeated authored
occurrences of one canonical edge are not.
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
when a coherent normalized model exists and no diagnostic fails the configured
validation policy. Warning escalation therefore suppresses the write. Non-failing
warning and information diagnostics may be included in the projection. The index
shall be a replaceable generated file and never an authoring input. A successful
write shall atomically replace the configured index with one complete canonical
document; validation or write failure shall never expose a truncated or partially
serialized index.
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
edges, derived source-span nodes and edges, inverse presentation, weak mentions,
external nodes, source spans, relation provenance, and available Git provenance.
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
:mitigates: RISK-INDEX-AUTHORITY

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

:::decision m_01KYDMFST0EY73VM8ZVCNZKHV5
:id: ADR-0014
:title: Defer Windows runtime qualification from bootstrap delivery
:status: accepted
:kind: process
:justifies: REQ-PORTABLE-CLI

Mara v0.1 bootstrap delivery shall proceed on non-Windows hosts while Windows
runtime support remains unclaimed and unverified. Windows is not a supported
Mara v0.1 runtime until the dedicated portability qualification has completed.
Incidental compilation, cross-compilation, or partial compatibility is not
verification evidence and does not constitute a support guarantee.

The dedicated Windows delivery shall establish native behavioural CI and close
the remaining platform-specific implementation and verification work before
Mara claims Windows runtime support. Deferring that qualification does not block
the current non-Windows bootstrap scope.
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

:::design m_01KZ7YA2D7EJ73VM8ZVCNZKHV5
:id: DES-V01-QUALIFICATION-METHOD
:title: v0.1 qualification architecture umbrella
:status: accepted
:kind: architecture
:satisfies: REQ-PERFORMANCE-TARGET
:refines: REQ-PORTABLE-CLI
:relates_to: TEST-PORTABILITY-PERFORMANCE

This proposed umbrella keeps the v0.1 scale and native non-Windows
qualification method coherent without changing the approved requirement, test,
or Windows boundary in [[ADR-0014]]. The focused command, external-storage,
fixture, oracle, measurement, evidence-format, and project-sandbox contracts
are defined in [v0.1 qualification](../reference/v01-qualification.mara.md).

Those contracts define reusable verification procedures and stable planned
outputs, not execution evidence or a runtime-support claim. Native Windows
behavioural qualification remains deferred by [[ADR-0014]]; Linux and macOS
results or Windows cross-compilation do not establish that support boundary.
:::

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
source, allowing stale or modified derived state to appear authoritative.
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
show mentions, human snapshots, and stable JSON data. Invalid filters and
unresolved or ambiguous `show` references shall produce `status: invalid` with
null `data` and null `error` while retaining exact diagnostics.
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
the same endpoints; they shall prove simple-path exclusion of repeated node
identities, absence of the zero-step focus path, occurrence deduplication, and
relation-based parallel-path preservation. Additional fixtures shall traverse
from an item to an external target and from an item backlink to a derived
source-span node, proving that non-item endpoint identities appear in path steps
and terminate safely. The derived node shall be injected at the normalized-model
fixture seam; this test does not require a configured source scanner.
Unresolved or ambiguous focus references shall produce `status: invalid` with
null `data` and null `error` while retaining exact diagnostics.
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
UTF-8 serialization, and absence of machine-specific absolute paths. Policy
fixtures shall prove that non-failing warnings are serialized, warning escalation
suppresses generation, every failing diagnostic leaves any existing index
unchanged, and a later successful rebuild atomically replaces it. Independent
normalized-model fixtures shall prove exact `source_nodes` projection, `NodeRef`
endpoint encoding, incoming item backlinks, deterministic deduplication, and
absence of invented MIDs for source spans. Scanner discovery and language-aware
marker recognition remain in the future-adapter test. Fault-injected writer
fixtures shall cover temporary serialization, file flush, atomic replacement,
and parent-directory flush, proving that the configured path contains the
complete previous or complete new index and never a partial document.
When validation policy suppresses an index write, the JSON command envelope
shall use `status: invalid`, null `data`, and null `error`, and the fixture shall
prove that no new index or hash is reported and any previous index is unchanged.
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

The dedicated portability qualification shall run behavioural suites on Linux,
macOS, and Windows before support is claimed for those runtimes. It shall also
benchmark the documented 10,000-item, 100,000-edge fixture on the designated
reference runner against time and memory budgets. Passing non-Windows CI or
cross-compilation alone shall not verify Windows runtime support.
:::

:::test m_01KYHDXJ5V9EWWGCFCNB5KK0JB
:id: TEST-V01-SCALE-QUALIFICATION
:title: Scale-v0.1 deterministic qualification procedure
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-PERFORMANCE-TARGET
:verifies: DES-V01-QUALIFICATION-CLI
:verifies: DES-V01-QUALIFICATION-WORKSPACE
:verifies: DES-V01-SCALE-FIXTURE
:verifies: DES-V01-SCALE-ORACLE
:verifies: DES-V01-MEASUREMENT-RUNNER
:verifies: DES-V01-QUALIFICATION-EVIDENCE-FORMAT
:depends_on: TEST-PORTABILITY-PERFORMANCE

On the documented native Linux reference runner, build the bounded Rust tool,
create the unique external qualification root, generate the fixed project, and
run the independent oracle exactly once immediately before run 1, retaining its
fixed evidence outputs unchanged through five exact-child measurements.
Verify the complete record set, hash manifest, source provenance, storage
preconditions, five passing runs, and maxima against the stated limits. Retain
the records and perform the required upload and finally cleanup for passed,
failed, and inconclusive outcomes.
:::

:::test m_01KYHDXJ6DPHS94EPM4WGTKKS8
:id: TEST-V01-NATIVE-NONWINDOWS-QUALIFICATION
:title: Native Linux and macOS qualification procedure
:status: approved
:kind: verification
:method: automated
:level: system
:verifies: REQ-PORTABLE-CLI
:verifies: DES-V01-QUALIFICATION-WORKSPACE
:verifies: DES-V01-SCALE-ORACLE
:depends_on: TEST-PORTABILITY-PERFORMANCE
:relates_to: ADR-0014

From a clean source worktree on native GitHub-hosted ubuntu-24.04 and macos-14,
derive a nonexistent job-unique root, record pre-resource state, assert Linux
or Darwin as applicable, use current stable Rust with CARGO_TARGET_DIR unset,
and run cargo fmt --all -- --check, cargo clippy --all-targets --all-features
-- -D warnings, the built qualification generator, and
TMPDIR="$qualification_root/test-tmp" cargo test --all-targets --all-features.
Delete that test workspace immediately, build the native release binary in the
source target directory, and run the external standalone fixture's storage
gates, no-Git oracle, and provenance capture. macOS stops after native
behavioural qualification; only the Linux x64 reference runner, after requiring
RUNNER_ARCH X64 and uname -m x86_64, performs the five-run performance
measurement. The procedure does not make a Windows runtime claim.
:::

:::test m_01KYHDXJ6Z9JDF9Y83X6GENF74
:id: TEST-PROJECT-SANDBOX
:title: ProjectSandbox isolation procedure
:status: approved
:kind: verification
:method: automated
:level: integration
:verifies: DES-PROJECT-SANDBOX
:relates_to: TEST-PORTABILITY-PERFORMANCE

Project-oriented integration tests shall exercise exactly the empty, configured,
clean-Git, and dirty-Git modes. They shall verify a canonical unique project
outside the source checkout and every worktree, explicit child current
directory, clearing of every inherited GIT_* variable, the exact canonical
sandbox-parent GIT_CEILING_DIRECTORIES value, default cleanup, deletion-error
reporting, and failure-only opt-in preservation. Parser, domain, and in-memory
tests shall demonstrably avoid this helper.
:::
